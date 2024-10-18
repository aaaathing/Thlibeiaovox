#[cxx::bridge]
mod ffi {
	unsafe extern "C++" {
		include!("thlibeiaovox/structures/voxeldag/storage.cpp");

		type DAG;

		fn new_dag() -> UniquePtr<DAG>;
		fn voxel(&self, x:i32,y:i32,z:i32) -> u8;
	}
}
type DAG = ffi::DAG;

pub fn new_dag() -> cxx::UniquePtr<DAG> { ffi::new_dag() }

/*
const Internals::NodeStore& nodeStore = Internals::getNodes(volume()).nodes();
nodeStore.data()
somename

SubDAGArray subDAGs = findSubDAGs(nodeStore, getRootNodeIndex(volume()));
someothername
*/

