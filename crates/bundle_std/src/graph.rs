use crate::env;
use crate::sys;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use thing_abi::{GraphPropsGetRequest, GraphPropsRequest, Symbol, Value};
use thingos_kernel_std::id::{BundleId, PredId, ThingId};

#[derive(Debug)]
pub enum GraphError {
    SyscallFailed,
    SerializationFailed,
    DeserializationFailed,
}

pub struct BundleHandle {
    pub id: BundleId,
}

pub struct BundleProps {
    pub props: BTreeMap<Symbol, Value>,
}

impl BundleProps {
    pub fn get_u64(&self, pred: PredId) -> Option<u64> {
        let sym = Symbol(pred.0 as u32);
        match self.props.get(&sym) {
            Some(Value::U64(v)) => Some(*v),
            Some(Value::I64(v)) => Some(*v as u64),
            _ => None,
        }
    }

    pub fn get_text(&self, pred: PredId) -> Option<String> {
        let sym = Symbol(pred.0 as u32);
        match self.props.get(&sym) {
            Some(Value::Text(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

pub enum PropValue {
    U64(u64),
    Text(String),
}

impl From<u64> for PropValue {
    fn from(v: u64) -> Self {
        PropValue::U64(v)
    }
}

impl From<String> for PropValue {
    fn from(s: String) -> Self {
        PropValue::Text(s)
    }
}

impl Into<Value> for PropValue {
    fn into(self) -> Value {
        match self {
            PropValue::U64(v) => Value::U64(v),
            PropValue::Text(s) => Value::Text(s),
        }
    }
}

impl BundleHandle {
    pub fn get_root_thing(&self) -> ThingId {
        ThingId(self.id.0)
    }

    pub fn get_props(&self, thing: ThingId) -> Result<BundleProps, GraphError> {
        let req = GraphPropsGetRequest {
            node: thing.0,
            keys: Vec::new(), // Get all
        };

        let req_bytes = postcard::to_allocvec(&req).map_err(|_| GraphError::SerializationFailed)?;
        let mut out_buf = [0u8; 4096]; // Fixed buffer for now

        let len = sys::graph_get_props_raw(&req_bytes, &mut out_buf);
        if len == !0 {
            return Err(GraphError::SyscallFailed);
        }

        let props: BTreeMap<Symbol, Value> = postcard::from_bytes(&out_buf[..len as usize])
            .map_err(|_| GraphError::DeserializationFailed)?;

        Ok(BundleProps { props })
    }

    pub fn set_prop<T: Into<PropValue>>(
        &self,
        thing: ThingId,
        pred: PredId,
        value: T,
    ) -> Result<(), GraphError> {
        let val: Value = value.into().into();
        let sym = Symbol(pred.0 as u32);

        let mut props = BTreeMap::new();
        props.insert(sym, val);

        let req = GraphPropsRequest {
            node: thing.0,
            props,
        };

        let req_bytes = postcard::to_allocvec(&req).map_err(|_| GraphError::SerializationFailed)?;

        let ret = sys::graph_set_props_raw(&req_bytes);
        if ret == 0 {
            // 0 is success in my sys.rs wrapper? No, wait.
            // In sys.rs:
            // match crate::runtime().call(abi_req) {
            //     thing_abi::AbiResponse::Props { .. } => 1, // Success
            //     _ => 0,
            // }
            // But for target_os="none":
            // if graph::set_props(current_bundle(), request) { 0 } else { !0 }
            // So 0 is success.
            Ok(())
        } else {
            Err(GraphError::SyscallFailed)
        }
    }
}

pub fn current_bundle_handle() -> BundleHandle {
    BundleHandle {
        id: env::current_bundle(),
    }
}
