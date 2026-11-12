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
/// Append one item and recursively save group records, even for unselected groups.
pub fn save_item<H: Host>(
    host: &mut H,
    scene: H::Handle,
    item: H::Handle,
    pass: &mut Pass<H::Handle>,
) {
    if host.selected(item) || pass.all_items {
        let data = host.create_data();
        let info = host.info(item);
        let crop = host.crop(item);
        let id = host.id(item);
        host.set_int(data, "id", id);
        host.set_vec2(data, "pos", info.pos);
        host.set_vec2(data, "scale", info.scale);
        host.set_double(data, "rot", f64::from(info.rot));
        host.set_int(data, "alignment", i64::from(info.alignment));
        host.set_int(data, "bounds_type", i64::from(info.bounds_type));
        host.set_vec2(data, "bounds", info.bounds);
        host.set_int(data, "bounds_alignment", i64::from(info.bounds_alignment));
        host.set_bool(data, "crop_to_bounds", info.crop_to_bounds);
        for (key, value) in [
            ("top", crop[1]),
            ("bottom", crop[3]),
            ("left", crop[0]),
            ("right", crop[2]),
        ] {
            host.set_int(data, key, i64::from(value));
        }
        host.push(pass.ids, data);
        host.release_data(data);
    }
    let source = host.item_source(item);
    if host.is_group(source) {
        let data = host.create_data();
        let ids = host.create_array();
        let name = host.name(source);
        host.set_text(data, "scene_name", name);
        let uuid = host.uuid(source);
        host.set_text(data, "scene_uuid", uuid);
        host.set_bool(data, "is_group", true);
        let parent = host.source(scene);
        let parent_uuid = host.uuid(parent);
        host.set_text(data, "group_parent", parent_uuid);
        let mut nested = Pass {
            ids,
            scenes_and_groups: pass.scenes_and_groups,
            all_items: pass.all_items,
        };
        host.enum_group(item, &mut nested);
        host.set_array(data, "items", ids);
        host.push(pass.scenes_and_groups, data);
        host.release_data(data);
        host.release_array(ids);
    }
}
/// Return an owned wrapper with depth-first group records followed by the root.
pub fn save<H: Host>(host: &mut H, scene: H::Handle, all_items: bool) -> H::Handle {
    let wrapper = host.create_data();
    let groups = host.create_array();
    let ids = host.create_array();
    let mut pass = Pass {
        ids,
        scenes_and_groups: groups,
        all_items,
    };
    let data = host.create_data();
    let source = host.source(scene);
    let name = host.name(source);
    host.set_text(data, "scene_name", name);
    let source = host.source(scene);
    let uuid = host.uuid(source);
    host.set_text(data, "scene_uuid", uuid);
    host.set_bool(data, "is_group", false);
    host.enum_scene(scene, &mut pass);
    host.set_array(data, "items", ids);
    host.push(groups, data);
    host.set_array(wrapper, "scenes_and_groups", groups);
    host.release_array(ids);
    host.release_array(groups);
    host.release_data(data);
    wrapper
}
/// Restore one item in a deferred update, normalizing bounds before narrowing.
pub fn load_item<H: Host>(host: &mut H, data: H::Handle, scene: H::Handle) {
    let id = host.int(data, "id");
    let item = host.find(scene, id);
    let pos = host.vec2(data, "pos");
    let scale = host.vec2(data, "scale");
    let rot = host.double(data, "rot") as f32;
    let alignment = host.int(data, "alignment") as u32;
    let bounds = host.int(data, "bounds_type");
    let bounds_type = if (0..=6).contains(&bounds) {
        bounds as u32
    } else {
        0
    };
    let bounds_alignment = host.int(data, "bounds_alignment") as u32;
    let bounds = host.vec2(data, "bounds");
    let crop_to_bounds = host.boolean(data, "crop_to_bounds");
    let info = Info {
        pos,
        scale,
        rot,
        alignment,
        bounds_type,
        bounds_alignment,
        bounds,
        crop_to_bounds,
    };
    let top = host.int(data, "top") as i32;
    let bottom = host.int(data, "bottom") as i32;
    let left = host.int(data, "left") as i32;
    let right = host.int(data, "right") as i32;
    host.defer_begin(item);
    host.set_info(item, info);
    host.set_crop(item, [left, top, right, bottom]);
    host.defer_end(item);
}
/// Resolve groups by parent scene UUID and group name. As in C, a group source
/// cannot act as the parent scene, so nested group children are not restored.
pub fn load_record<H: Host>(host: &mut H, data: H::Handle) {
    let items = host.array(data, "items");
    let uuid = host.text(data, "scene_uuid");
    let source = host.lookup(uuid);
    let mut scene = host.scene(source);
    if host.boolean(data, "is_group") {
        let parent_uuid = host.text(data, "group_parent");
        let parent_source = host.lookup(parent_uuid);
        let parent = host.scene(parent_source);
        let name = host.text(data, "scene_name");
        let group = host.group(parent, name);
        scene = host.group_scene(group);
        host.release_source(parent_source);
    }
    host.enum_data(items, scene, false);
    host.release_array(items);
    host.release_source(source);
}
/// Parse and restore all records; missing arrays and malformed JSON enumerate nothing.
pub fn load<H: Host>(host: &mut H, json: H::Text) {
    let data = host.parse(json);
    let groups = host.array(data, "scenes_and_groups");
    host.enum_data(groups, host.null(), true);
    host.release_data(data);
    host.release_array(groups);
}
