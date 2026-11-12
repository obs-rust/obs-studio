//! Scene transform snapshot/restore policy. Handles and ownership stay with the host.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Info {
    pub pos: [f32; 2],
    pub rot: f32,
    pub scale: [f32; 2],
    pub alignment: u32,
    pub bounds_type: u32,
    pub bounds_alignment: u32,
    pub bounds: [f32; 2],
    pub crop_to_bounds: bool,
}
/// Crop order matches the public API: left, top, right, bottom.
pub type Crop = [i32; 4];
/// Borrowed scene/item handles and explicitly owned data/array/source references.
/// Enumeration holds the same host locks as the public scene/data APIs.
pub trait Host: Sized {
    type Handle: Copy;
    type Text;
    fn create_data(&mut self) -> Self::Handle;
    fn create_array(&mut self) -> Self::Handle;
    fn parse(&mut self, json: Self::Text) -> Self::Handle;
    fn release_data(&mut self, data: Self::Handle);
    fn release_array(&mut self, array: Self::Handle);
    fn text(&mut self, data: Self::Handle, key: &'static str) -> Self::Text;
    fn set_text(&mut self, data: Self::Handle, key: &'static str, text: Self::Text);
    fn int(&mut self, data: Self::Handle, key: &'static str) -> i64;
    fn set_int(&mut self, data: Self::Handle, key: &'static str, value: i64);
    fn double(&mut self, data: Self::Handle, key: &'static str) -> f64;
    fn set_double(&mut self, data: Self::Handle, key: &'static str, value: f64);
    fn boolean(&mut self, data: Self::Handle, key: &'static str) -> bool;
    fn set_bool(&mut self, data: Self::Handle, key: &'static str, value: bool);
    fn vec2(&mut self, data: Self::Handle, key: &'static str) -> [f32; 2];
    fn set_vec2(&mut self, data: Self::Handle, key: &'static str, value: [f32; 2]);
    fn array(&mut self, data: Self::Handle, key: &'static str) -> Self::Handle;
    fn set_array(&mut self, data: Self::Handle, key: &'static str, array: Self::Handle);
    fn push(&mut self, array: Self::Handle, data: Self::Handle);
    fn enum_scene(&mut self, scene: Self::Handle, pass: &mut Pass<Self::Handle>);
    fn enum_group(&mut self, item: Self::Handle, pass: &mut Pass<Self::Handle>);
    fn enum_data(&mut self, array: Self::Handle, scene: Self::Handle, groups: bool);
    fn source(&mut self, scene: Self::Handle) -> Self::Handle;
    fn item_source(&mut self, item: Self::Handle) -> Self::Handle;
    fn is_group(&mut self, source: Self::Handle) -> bool;
    fn name(&mut self, source: Self::Handle) -> Self::Text;
    fn uuid(&mut self, source: Self::Handle) -> Self::Text;
    fn selected(&mut self, item: Self::Handle) -> bool;
    fn id(&mut self, item: Self::Handle) -> i64;
    fn info(&mut self, item: Self::Handle) -> Info;
    fn crop(&mut self, item: Self::Handle) -> Crop;
    fn lookup(&mut self, uuid: Self::Text) -> Self::Handle;
    fn scene(&mut self, source: Self::Handle) -> Self::Handle;
    fn group(&mut self, parent: Self::Handle, name: Self::Text) -> Self::Handle;
    fn group_scene(&mut self, item: Self::Handle) -> Self::Handle;
    fn release_source(&mut self, source: Self::Handle);
    fn find(&mut self, scene: Self::Handle, id: i64) -> Self::Handle;
    fn defer_begin(&mut self, item: Self::Handle);
    fn set_info(&mut self, item: Self::Handle, info: Info);
    fn set_crop(&mut self, item: Self::Handle, crop: Crop);
    fn defer_end(&mut self, item: Self::Handle);
    fn null(&self) -> Self::Handle;
}
/// Callback state shared with recursive group enumerations.
pub struct Pass<H> {
    pub ids: H,
    pub scenes_and_groups: H,
    pub all_items: bool,
}
pub fn save_item<H: Host>(
    _host: &mut H,
    _scene: H::Handle,
    _item: H::Handle,
    _pass: &mut Pass<H::Handle>,
) {
    todo!("scene transform snapshot")
}
pub fn save<H: Host>(_host: &mut H, _scene: H::Handle, _all_items: bool) -> H::Handle {
    todo!("scene transform snapshot")
}
pub fn load_item<H: Host>(_host: &mut H, _data: H::Handle, _scene: H::Handle) {
    todo!("scene transform restore")
}
pub fn load_record<H: Host>(_host: &mut H, _data: H::Handle) {
    todo!("scene transform restore")
}
pub fn load<H: Host>(_host: &mut H, _json: H::Text) {
    todo!("scene transform restore")
}
