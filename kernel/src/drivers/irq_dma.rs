use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex as SpinMutex;
use uuid::Uuid;
use x86_64::instructions::interrupts;

use crate::graph::{self, BundleId, GraphFiatRequest, canon};
use thing_abi::Value;

pub type IrqHandle = u64;
pub type DmaMappingHandle = u64;
pub type DmaHandle = u64;

#[derive(Clone)]
struct IrqBinding {
    handle: IrqHandle,
    bundle: BundleId,
    device: Uuid,
    irq_line: u8,
}

#[derive(Clone)]
pub struct IrqBindingInfo {
    pub bundle: BundleId,
    pub device: Uuid,
    pub irq_line: u8,
}

#[derive(Default)]
struct IrqState {
    next_handle: IrqHandle,
    bindings: Vec<IrqBinding>,
}

impl IrqState {
    fn bind(&mut self, bundle: BundleId, device: Uuid, irq_line: u8) -> Option<IrqHandle> {
        if !graph::bundle_has_capability(bundle, device, canon::CAN_HANDLE_IRQ) {
            return None;
        }
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        self.bindings.push(IrqBinding {
            handle,
            bundle,
            device,
            irq_line,
        });
        Some(handle)
    }

    fn binding(&self, handle: IrqHandle) -> Option<&IrqBinding> {
        self.bindings.iter().find(|b| b.handle == handle)
    }

    fn by_line(&self, irq_line: u8) -> impl Iterator<Item = &IrqBinding> {
        self.bindings.iter().filter(move |b| b.irq_line == irq_line)
    }

    fn unregister(&mut self, handle: IrqHandle) -> bool {
        if let Some(pos) = self.bindings.iter().position(|b| b.handle == handle) {
            self.bindings.remove(pos);
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct DmaMapping {
    handle: DmaMappingHandle,
    bundle: BundleId,
    buffer: Uuid,
    flags: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DmaStatus {
    Pending,
    Complete,
}

#[derive(Clone)]
pub(crate) struct DmaSubmission {
    handle: DmaHandle,
    bundle: BundleId,
    device: Uuid,
    mapping: DmaMappingHandle,
    bytes: u64,
    status: DmaStatus,
}

#[derive(Default)]
struct DmaState {
    next_mapping: DmaMappingHandle,
    next_submit: DmaHandle,
    mappings: BTreeMap<DmaMappingHandle, DmaMapping>,
    submissions: BTreeMap<DmaHandle, DmaSubmission>,
}

impl DmaState {
    fn map(&mut self, bundle: BundleId, buffer: Uuid, flags: u64) -> Option<DmaMappingHandle> {
        if !graph::bundle_has_capability(bundle, buffer, canon::CAN_DMA) {
            return None;
        }
        let handle = self.next_mapping;
        self.next_mapping = self.next_mapping.wrapping_add(1).max(1);
        let mapping = DmaMapping {
            handle,
            bundle,
            buffer,
            flags,
        };
        self.mappings.insert(handle, mapping);
        Some(handle)
    }

    fn submit(
        &mut self,
        bundle: BundleId,
        device: Uuid,
        mapping: DmaMappingHandle,
        bytes: u64,
    ) -> Option<DmaHandle> {
        let Some(mapping) = self.mappings.get(&mapping) else {
            return None;
        };
        if mapping.bundle != bundle {
            return None;
        }
        if !graph::bundle_has_capability(bundle, device, canon::CAN_DMA) {
            return None;
        }
        let handle = self.next_submit;
        self.next_submit = self.next_submit.wrapping_add(1).max(1);
        let submission = DmaSubmission {
            handle,
            bundle,
            device,
            mapping: mapping.handle,
            bytes,
            status: DmaStatus::Pending,
        };
        self.submissions.insert(handle, submission);
        Some(handle)
    }

    fn wait(&mut self, bundle: BundleId, handle: DmaHandle) -> Option<DmaSubmission> {
        let Some(submission) = self.submissions.get_mut(&handle) else {
            return None;
        };
        if submission.bundle != bundle {
            return None;
        }
        submission.status = DmaStatus::Complete;
        Some(submission.clone())
    }
}

static IRQ_STATE: SpinMutex<IrqState> = SpinMutex::new(IrqState {
    next_handle: 1,
    bindings: Vec::new(),
});

static DMA_STATE: SpinMutex<DmaState> = SpinMutex::new(DmaState {
    next_mapping: 1,
    next_submit: 1,
    mappings: BTreeMap::new(),
    submissions: BTreeMap::new(),
});

pub fn register_irq(bundle: BundleId, device: Uuid, irq_line: u8) -> Option<IrqHandle> {
    let mut state = IRQ_STATE.lock();
    state.bind(bundle, device, irq_line)
}

pub fn unregister_irq(handle: IrqHandle) -> bool {
    let mut state = IRQ_STATE.lock();
    state.unregister(handle)
}

pub fn irq_bind(bundle: BundleId, device: Uuid, irq_line: u8) -> Option<IrqHandle> {
    register_irq(bundle, device, irq_line)
}

pub fn irq_ack(_bundle: BundleId, _handle: IrqHandle) -> bool {
    // Previously this unregistered the IRQ, but that caused the driver to lose
    // the binding after the first event. For now, we just return true to
    // indicate success, which triggers the kernel to process events.
    true
}

pub fn bindings_for_irq(irq_line: u8) -> Vec<IrqBindingInfo> {
    let state = IRQ_STATE.lock();
    state
        .by_line(irq_line)
        .map(|b| IrqBindingInfo {
            bundle: b.bundle,
            device: b.device,
            irq_line: b.irq_line,
        })
        .collect()
}

pub fn for_each_binding<F>(irq_line: u8, mut f: F)
where
    F: FnMut(&IrqBindingInfo),
{
    let state = IRQ_STATE.lock();
    for binding in state.by_line(irq_line) {
        let info = IrqBindingInfo {
            bundle: binding.bundle,
            device: binding.device,
            irq_line: binding.irq_line,
        };
        f(&info);
    }
}
fn emit_irq_event(binding: &IrqBinding) {
    let mut fields = BTreeMap::new();
    fields.insert(canon::DEVICE_ID, Value::Uuid(binding.device));
    fields.insert(canon::IRQ_LINE, Value::U64(binding.irq_line as u64));
    fields.insert(canon::TS, Value::U64(ticks_since_boot_saturating()));

    let req = GraphFiatRequest {
        id: None,
        kind: canon::IRQ_EVENT,
        labels: vec![canon::IRQ_EVENT],
        fields,
    };
    let _ = graph::fiat_for_bundle(binding.bundle, req);
}

pub fn dma_map(bundle: BundleId, buffer: Uuid, flags: u64) -> Option<DmaMappingHandle> {
    DMA_STATE.lock().map(bundle, buffer, flags)
}

pub fn dma_submit(
    bundle: BundleId,
    device: Uuid,
    mapping: DmaMappingHandle,
    bytes: u64,
) -> Option<DmaHandle> {
    DMA_STATE.lock().submit(bundle, device, mapping, bytes)
}

pub fn dma_wait(bundle: BundleId, handle: DmaHandle) -> Option<DmaSubmission> {
    let result = DMA_STATE.lock().wait(bundle, handle);
    if let Some(ref submission) = result {
        emit_dma_event(submission);
    }
    result
}

fn emit_dma_event(submission: &DmaSubmission) {
    let mut fields = BTreeMap::new();
    fields.insert(canon::DEVICE_ID, Value::Uuid(submission.device));
    fields.insert(canon::BUFFER, Value::U64(submission.mapping));
    fields.insert(canon::BYTES, Value::U64(submission.bytes));
    fields.insert(canon::STATUS, Value::Symbol(canon::DONE));

    let req = GraphFiatRequest {
        id: None,
        kind: canon::DMA_EVENT,
        labels: vec![canon::DMA_EVENT],
        fields,
    };
    let _ = graph::fiat_for_bundle(submission.bundle, req);
}

fn ticks_since_boot_saturating() -> u64 {
    unsafe {
        crate::clock::CLOCK
            .map(|clock| clock.lock().ticks_since_boot())
            .unwrap_or(0)
    }
}

pub fn monotonic_ticks() -> u64 {
    ticks_since_boot_saturating()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{self, BundleType};

    fn new_id(name: &str) -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes())
    }

    fn ensure_bundle(name: &str, ty: BundleType) -> BundleId {
        let id = new_id(name);
        graph::create_package_with_id(BundleId(id), name, ty, None)
    }

    #[test]
    fn irq_binding_requires_capability() {
        graph::init();
        let device = new_id("irq-device");
        let bundle = ensure_bundle("irq-bundle", BundleType::Driver);
        let handle_missing = irq_bind(bundle, device, 1);
        assert!(handle_missing.is_none());
        graph::grant_initial_capability(bundle, device, canon::CAN_HANDLE_IRQ);
        let handle = irq_bind(bundle, device, 1);
        assert!(handle.is_some());
    }

    #[test]
    fn dma_map_and_submit_respects_capabilities() {
        graph::init();
        let buffer = new_id("buffer");
        let device = new_id("device");
        let bundle = ensure_bundle("dma-bundle", BundleType::Driver);

        let map_missing = dma_map(bundle, buffer, 0);
        assert!(map_missing.is_none());
        graph::grant_initial_capability(bundle, buffer, canon::CAN_DMA);
        graph::grant_initial_capability(bundle, device, canon::CAN_DMA);

        let mapping = dma_map(bundle, buffer, 0).expect("mapping");
        let handle = dma_submit(bundle, device, mapping, 128).expect("submit");
        let submission = dma_wait(bundle, handle).expect("wait");
        assert_eq!(submission.status, DmaStatus::Complete);
    }
}
