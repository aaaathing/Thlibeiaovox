mod structures;

fn main(){
	println!("stuff");
	let thing = structures::voxeldag::new_dag();
	println!("{}",thing.voxel(1i32,2i32,3i32));
}