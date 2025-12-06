use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec::Vec;
use userland::canon;
use userland::flex::{AlignItems, FlexDirection, JustifyContent};
use userland::graph;
use userland::Value;
use uuid::Uuid;

pub struct TemplateBuilder {
    template_id: Uuid,
}

impl TemplateBuilder {
    pub fn new(name: &str, role: &str, mode: Option<i64>) -> Self {
        let template_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
        let mut fields = BTreeMap::new();
        fields.insert(canon::KIND, Value::Symbol(canon::LAYOUT_TEMPLATE));
        fields.insert(canon::ROLE, Value::Text(role.to_string()));
        if let Some(m) = mode {
            fields.insert(canon::MODE, Value::I64(m));
        }
        graph::fiat(Some(template_id), canon::LAYOUT_TEMPLATE, fields);
        Self { template_id }
    }

    pub fn add_region(&self, region: RegionBuilder) -> &Self {
        let region_id = region.build();
        graph::that(self.template_id, "contains", region_id, 0);
        self
    }

    pub fn id(&self) -> Uuid {
        self.template_id
    }
}

pub struct RegionBuilder {
    id: Uuid,
    fields: BTreeMap<canon::Symbol, Value>,
    children: Vec<RegionBuilder>,
}

impl RegionBuilder {
    pub fn new(name: &str) -> Self {
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
        let mut fields = BTreeMap::new();
        fields.insert(canon::KIND, Value::Symbol(canon::LAYOUT_REGION));
        Self {
            id,
            fields,
            children: Vec::new(),
        }
    }

    pub fn kind(mut self, kind: &str) -> Self {
        self.fields
            .insert(canon::WIDGET_KIND, Value::Text(kind.to_string()));
        self
    }

    pub fn role(mut self, role: &str) -> Self {
        self.fields
            .insert(canon::ROLE, Value::Text(role.to_string()));
        self
    }

    pub fn flex_dir(mut self, dir: FlexDirection) -> Self {
        self.fields.insert(canon::FLEX_DIRECTION, dir.to_value());
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.fields.insert(canon::cc('J', 'C'), justify.to_value());
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.fields.insert(canon::cc('A', 'I'), align.to_value());
        self
    }

    pub fn gap(mut self, gap: i64) -> Self {
        self.fields.insert(canon::GAP, Value::I64(gap));
        self
    }

    pub fn width(mut self, w: u64) -> Self {
        self.fields.insert(canon::WIDTH, Value::U64(w));
        self
    }

    pub fn height(mut self, h: u64) -> Self {
        self.fields.insert(canon::HEIGHT, Value::U64(h));
        self
    }

    pub fn grow(mut self, g: i64) -> Self {
        self.fields.insert(canon::FLEX_GROW, Value::I64(g));
        self
    }

    pub fn shrink(mut self, s: i64) -> Self {
        self.fields.insert(canon::FLEX_SHRINK, Value::I64(s));
        self
    }

    pub fn binds_to(mut self, name: &str) -> Self {
        self.fields
            .insert(canon::BINDS_TO, Value::Text(name.to_string()));
        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.fields
            .insert(canon::TEXT, Value::Text(text.to_string()));
        self
    }

    pub fn prop(mut self, key: canon::Symbol, value: Value) -> Self {
        self.fields.insert(key, value);
        self
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.fields
            .insert(canon::ICON_NAME, Value::Text(icon.to_string()));
        self
    }

    pub fn target(mut self, target: &str) -> Self {
        self.fields
            .insert(canon::TARGET, Value::Text(target.to_string()));
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.fields.insert(canon::FOCUSABLE, Value::Bool(focusable));
        self
    }

    pub fn child(mut self, child: RegionBuilder) -> Self {
        self.children.push(child);
        self
    }

    pub fn build(self) -> Uuid {
        let mut fields = self.fields;
        let mut children_ids = Vec::new();
        for child in self.children {
            children_ids.push(Value::Uuid(child.build()));
        }
        if !children_ids.is_empty() {
            fields.insert(canon::CONTAINS, Value::List(children_ids));
        }
        graph::fiat(Some(self.id), canon::LAYOUT_REGION, fields);
        self.id
    }
}
