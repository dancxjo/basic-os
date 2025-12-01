use thing_abi::ThingId;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct GraphHandle;

#[derive(Clone, Debug)]
pub struct WidgetEventRx;

#[derive(Clone, Debug)]
pub struct SharedFramebuffer {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    // In the future, this will hold a handle to shared memory.
    // For now, the buffer is passed to draw().
}

pub struct WidgetContext {
    pub widget_id: ThingId,
    pub graph: GraphHandle,
    pub events: WidgetEventRx,
    pub framebuffer: SharedFramebuffer,
}

#[derive(Clone, Debug)]
pub enum WidgetEvent {
    Paint,
    Input(InputEvent),
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    MouseMove { x: i32, y: i32 },
    MouseDown { x: i32, y: i32, button: u8 },
    MouseUp { x: i32, y: i32, button: u8 },
    KeyDown { key: u32 },
    KeyUp { key: u32 },
}

pub trait WidgetAbi {
    type State;

    fn init(ctx: &WidgetContext) -> Self::State;
    fn draw(state: &Self::State, fb: &mut [u8], rect: Rect);
    fn handle_event(state: &mut Self::State, event: WidgetEvent);
    fn teardown(state: Self::State);
}
