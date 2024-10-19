pub mod voxeldag;
use bvh_arena::{Bvh, volumes::Aabb, VolumeHandle};

pub struct World{
	stuff:Bvh<voxeldag::DAG, Aabb<3>>,
}
impl World{
	pub fn new() -> Self {
		let bvh = Bvh::default();
		Self {stuff:bvh}
	}
	pub fn add(&mut self) -> VolumeHandle {
		return self.stuff.insert(voxeldag::new_dag(), Aabb::from_min_max([0.,0.,0.],[0.,0.,0.]));
	}
	pub fn remove(&mut self, h:VolumeHandle){
		self.stuff.remove(h)
	}
}