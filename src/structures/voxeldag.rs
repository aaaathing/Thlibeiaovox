use autocxx::prelude::*; // use all the main autocxx functions

include_cpp! {
	#include "voxeldag/storage.cpp"
	safety!(unsafe)
	generate!("Cubiquity::Volume")
}

pub type DAG = UniquePtr<ffi::Cubiquity::Volume>;
pub fn new_dag() -> DAG {
	ffi::Cubiquity::Volume::new().within_unique_ptr()
}

/*
const Internals::NodeStore& nodeStore = Internals::getNodes(volume()).nodes();
nodeStore.data()
somename

SubDAGArray subDAGs = findSubDAGs(nodeStore, getRootNodeIndex(volume()));
someothername
*/

