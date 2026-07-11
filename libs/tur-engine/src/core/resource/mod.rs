mod image_resource;

pub use image_resource::*;

use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(u64);

impl ResourceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Default)]
pub struct ResourceMap {
    resources: HashMap<ResourceId, Resource>,
    next_id: u64,
}

impl fmt::Debug for ResourceMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceMap")
            .field("count", &self.resources.len())
            .finish()
    }
}

enum Resource {
    Image(ImageResource),
}

impl ResourceMap {
    pub fn insert_image(&mut self, image: ImageResource) -> ResourceId {
        let id = ResourceId(self.next_id);
        self.next_id += 1;
        self.resources.insert(id, Resource::Image(image));
        id
    }

    pub fn get_image(&self, id: ResourceId) -> Option<&ImageResource> {
        match self.resources.get(&id)? {
            Resource::Image(img) => Some(img),
        }
    }

    /// Iterate over all registered image resources with their ids.
    pub fn iter_images(&self) -> impl Iterator<Item = (ResourceId, &ImageResource)> {
        self.resources.iter().map(|(id, resource)| match resource {
            Resource::Image(img) => (*id, img),
        })
    }

    /// Whether an image resource with the given id is registered.
    pub fn has_image(&self, id: ResourceId) -> bool {
        matches!(self.resources.get(&id), Some(Resource::Image(_)))
    }
}
