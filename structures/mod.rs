pub mod voxeldag;
use bvh_arena::{Bvh, volumes::Aabb, VolumeHandle};

pub struct World{
	Bvh<u32, Aabb<3>> stuff,
}
impl World{
	pub fn add(&mut self){
		self.stuff.insert(voxeldag::new_dag(), Aabb.from_min_max([0,0,0],[0,0,0]));
	}
	pub fn remove(&mut self, VolumeHandle h){
		self.stuff.remove(h)
	}
}