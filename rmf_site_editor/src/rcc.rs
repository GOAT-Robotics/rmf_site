use crate::{
    site::{AnchorBundle, DrawingBundle, LoadResult},
    site_mode, JsValue, WorkspaceMarker,
};
use bevy::prelude::{Commands, SpatialBundle};
use once_cell::sync::Lazy;
use rmf_site_format::{
    Anchor, Category, DrawingProperties, Location, LocationTag, LocationTags, NameInSite, SiteID,
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::RangeFrom,
};
use tracing::error;

use serde::Deserialize;

// RCC Map source related logics
pub static mut MAP_INDEX: u32 = 0;

// Display whether map dropdown or text box on selecting "RCC" AssetSource
pub static mut SHOW_MAP_ASSET_SOURCE: u32 = 0;

#[derive(Debug, Deserialize, PartialEq)]
pub struct YamlData {
    pub mode: String,
    pub image: String,
    pub negate: u8,
    pub origin: [f32; 3],
    pub resolution: f32,
    pub free_thresh: f64,
    pub occupied_thresh: f64,
}

use serde::Serialize;

// Struct for Position
#[derive(Serialize, Deserialize, Debug)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// Struct for Orientation
#[derive(Serialize, Deserialize, Debug)]
pub struct Orientation {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// Struct for Pose, which includes Position and Orientation
#[derive(Serialize, Deserialize, Debug)]
pub struct Pose {
    pub position: Position,
    pub orientation: Orientation,
}

// Struct for Marker, which includes id, name, Pose, is_charger, and markerType
#[derive(Serialize, Deserialize, Debug)]
pub struct Marker {
    pub id: String,
    pub name: String,
    pub pose: Pose,
    pub marker_type: Vec<String>, // Empty array in your example, but it's a Vec of Strings
}

// Top-level structure for the list of markers
#[derive(Serialize, Deserialize, Debug)]
pub struct Markers {
    pub markers: Vec<Marker>,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub height: f32,
    pub width: f32,
}

#[derive(Debug, Deserialize)]
pub struct Maps {
    pub id: String,
    pub name: String,
    pub image_url: String,
    pub metadata: Metadata,
    pub yaml_data: YamlData,
    pub markers: Vec<Marker>,
}

pub fn set_selected_map_index(map_index: u32) {
    unsafe { MAP_INDEX = map_index };
}

pub fn parse_js_value(val: &JsValue) -> Result<Maps, Box<dyn std::error::Error>> {
    let curr_map_str = js_sys::JSON::stringify(&val)
        .unwrap()
        .as_string()
        .ok_or("Invalid string")?;
    let cur_map_obj: Maps = serde_json::from_str(&curr_map_str)?;
    Ok(cur_map_obj)
}

// Site Mode Related logics
pub static mut SITE_MODE: String = String::new();

pub fn set_site_mode() {
    let js_value: JsValue = site_mode().into();
    let rust_string: String = js_value.as_string().unwrap_or_default();
    unsafe { SITE_MODE = rust_string.to_string() }
}

pub fn is_site_in_view_mode() -> bool {
    return unsafe { &SITE_MODE } == "VIEW_MODE";
}

//Robot list Related logics
static mut ROBOT_LIST: Lazy<HashMap<u32, String>> = Lazy::new(|| {
    let map = HashMap::new();
    map
});

pub fn parse_robot_data(val: &JsValue) -> Result<String, Box<dyn std::error::Error>> {
    let curr_robot = js_sys::JSON::stringify(&val)
        .unwrap()
        .as_string()
        .ok_or("Invalid string")?;
    let cur_robot_obj: String = serde_json::from_str(&curr_robot)?;
    Ok(cur_robot_obj)
}

pub fn add_robot_in_robot_list(id: &str, index: u32) {
    unsafe {
        ROBOT_LIST.insert(index, id.to_string());
    }
}

pub fn get_robot_id(index: u32) -> Option<String> {
    unsafe { ROBOT_LIST.get(&index).cloned() }
}

// Robot pose Related logics
#[derive(Debug, Deserialize, PartialEq)]
pub struct RobotPose {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub yaw: f32,
    pub level_name: String,
}

static mut ROBOT_POSE: Lazy<HashMap<String, RobotPose>> = Lazy::new(|| {
    let map = HashMap::new();
    map
});

pub fn parse_robot_pose(val: &JsValue) -> Result<RobotPose, Box<dyn std::error::Error>> {
    let curr_map_str = js_sys::JSON::stringify(&val)
        .unwrap()
        .as_string()
        .ok_or("Invalid string")?;
    let cur_map_obj: RobotPose = serde_json::from_str(&curr_map_str)?;
    Ok(cur_map_obj)
}

pub fn get_robot_pose_by_id(id: &str) -> Option<&RobotPose> {
    unsafe { ROBOT_POSE.get(id) }
}

pub fn add_robot_pose_by_id(id: String, pose: RobotPose) {
    unsafe {
        ROBOT_POSE.insert(id, pose);
    }
}

pub fn load_milestones(map: Maps, level: &mut RangeFrom<u32>, commands: &mut Commands) {
    #[cfg(target_arch = "wasm32")]
    {
        crate::log(&format!(
            "Loading {} markers from {}",
            map.markers.len(),
            map.name
        ));
    }
    let site_id = commands
        .spawn(SpatialBundle::HIDDEN_IDENTITY)
        .insert(Category::Site)
        .insert(WorkspaceMarker)
        .id();

    commands
        .spawn(DrawingBundle::new(DrawingProperties {
            name: NameInSite(map.name.clone()),
            pixels_per_meter: rmf_site_format::PixelsPerMeter(20.0),
            source: rmf_site_format::AssetSource::RCC(map.image_url.clone()),
            ..Default::default()
        }))
        .insert(SiteID(level.next().unwrap()));

    let mut anchors: BTreeMap<u32, Anchor> = BTreeMap::new();
    let mut locations = BTreeMap::new();

    map.markers.iter().for_each(|marker| {
        let anchor = level.next().unwrap();
        let x_value = (marker.pose.position.x - map.yaml_data.origin[0]) + 0.20; //add 0.20 to x value for marker icon width;
        let y_offset = map.yaml_data.origin[1] + (map.metadata.height * map.yaml_data.resolution);
        let y_value = marker.pose.position.y - y_offset;

        anchors.insert(anchor, [x_value, y_value].into());

        let mut location = Location {
            name: NameInSite(marker.name.clone()),
            graphs: rmf_site_format::AssociatedGraphs::All,
            anchor: rmf_site_format::Point(anchor),
            tags: LocationTags::default(),
        };

        marker.marker_type.iter().for_each(|marker_type| {
            if marker_type.to_uppercase() == "CHARGER" {
                location.tags.push(LocationTag::Charger);
            } else if marker_type.to_uppercase() == "PARKING" {
                location.tags.push(LocationTag::ParkingSpot);
            } else if marker_type.to_uppercase() == "HOLDING" {
                location.tags.push(LocationTag::HoldingPoint);
            }
        });

        locations.insert(level.next().unwrap(), location);
    });

    let mut id_to_entity = HashMap::new();

    for (anchor_id, anchor) in &anchors {
        let anchor_entity = commands
            .spawn(AnchorBundle::new(anchor.clone()))
            .insert(SiteID(*anchor_id))
            .id();
        id_to_entity.insert(*anchor_id, anchor_entity);
    }

    for (location_id, location_data) in &locations {
        let entity = match location_data.convert(&id_to_entity).for_site(site_id) {
            Ok(e) => e,
            Err(err) => return error!("Failed to convert location {:?}", err),
        };
        commands.spawn(entity).insert(SiteID(*location_id));
    }
}
