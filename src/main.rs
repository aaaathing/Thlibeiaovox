// this file for testin
use thlibeiaovox_lib::structures;

fn main(){
	println!("stuff");
	let mut thing = structures::voxeldag::new_dag();
	thing.pin_mut().setVoxel(1,2,3, 123);
	thing.pin_mut().setVoxel(6,0,4, 89);
	println!("{}",thing.voxel(1,2,3));
	println!("{}",thing.voxel(6,0,4));
	println!("{} nodes",thing.countNodes());
	
}