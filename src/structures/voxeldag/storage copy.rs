/***************************************************************************************************
* Cubiquity - A micro-voxel engine for games and other interactive applications                    *
*                                                                                                  *
* Written in 2019 by David Williams                                                                *
*                                                                                                  *
* To the extent possible under law, the author(s) have dedicated all copyright and related and     *
* neighboring rights to this software to the public domain worldwide. This software is distributed *
* without any warranty.                                                                            *
*                                                                                                  *
* You should have received a copy of the CC0 Public Domain Dedication along with this software.    *
* If not, see http://creativecommons.org/publicdomain/zero/1.0/.                                   *
***************************************************************************************************/

#pragma once
#include "base.cpp"
#include "geometry.cpp"
#include "storage.h"

#include <algorithm>
#include <cassert>
#include <cstring>
#include <fstream>

//([a-zA-Z<>:*0-9_, ]+) (\w+) =
//(?<=[\(,] *)([a-zA-Z<>:*0-9_, &]+) (\w+)(?= *[\),])

mod Cubiquity
{
	type MaterialId = u16;
	const MinMaterial: MaterialId = 0;
	const MaxMaterial: MaterialId = -1;
	const MaterialCount: u32 = (MaxMaterial as u32) + 1;
	mod Internals
	{
		const VolumeSideLength: uint64 = 1u64 << 32;

		bool isMaterialNode(nodeIndex: u32);

		type Node = std::array<uint32_t, 8>;

		struct NodeDAG{
			mNodes: Vec<Node>,
			mBakedNodesEnd: u32,
			mEditNodesBegin: u32,
		}
		impl Index<u32> for NodeDAG{
			type Output = &Node;
			fn index(&self, idx:u32){
				return self.mNodes[idx];
			}
		}
		impl NodeDAG
		{
			uint32 bakedNodesBegin() const { return MaterialCount; }
			uint32 bakedNodesEnd() const { return mBakedNodesEnd; }
			uint32 editNodesBegin() const { return mEditNodesBegin; }
			uint32 editNodesEnd() const { return mNodes.size(); }

			bool isBakedNode(index: u32) const { return index >= bakedNodesBegin() && index < bakedNodesEnd(); }
			bool isEditNode(index: u32) const { return index >= editNodesBegin() && index < editNodesEnd(); }

			NodeStore& nodes() { return mNodes; }
			const NodeStore& nodes() const { return mNodes; }
		};
		impl Default for NodeStore {
			fn default() -> NodeStore {
				NodeStore{
					mBakedNodesEnd: MaterialCount,
					editNodesBegin: 0
				}
			}
		}

		fn isMaterialNode(uint32 nodeIndex) { return nodeIndex < MaterialCount; }
	}

	class Brush
	{
	public:
		virtual bool contains(const Vector3f& point) const = 0;
		virtual Box3f bounds() const = 0;

		Vector3f mCentre;
	};

	struct Volume{
		mDAG: Internals::NodeDAG,

		// Note: The undo system is linear. If we apply operation A, undo it, and then apply operation B
		// then A is lost. But it we wanted to we could still track it, because the appropriate edit still
		// exists (it is jst unreferenced). But having such an undo history tree will probably be confusing
		// for the user, moving forwards and backwards is probably enough.
		mTrackEdits: bool = false,
		mRootNodeIndices: Vec<uint32>,
		mCurrentRoot: uint32 = 0,
	}
	impl Volume
	{
		uint32 countNodes() const { return mDAG.countNodes(rootNodeIndex()); };
	};

	// See https://stackoverflow.com/a/57796299
	constexpr Node makeNode(uint32 value)
	{
		Node node{};
		for (auto& x : node) { x = value; }
		return node;
	}

	void NodeStore::setNode(uint32 index, const Node& newNode)
	{
		assert(!isMaterialNode(index) && "Error - Cannot modify material nodes");

		for (const uint32& childIndex : newNode)
		{
			assert(childIndex != index && "Error - Child points at parent");
		}

		mData[index] = newNode;
	}

	void NodeStore::setNodeChild(uint32 nodeIndex, uint32 childId, uint32 newChildIndex)
	{
		assert(!isMaterialNode(nodeIndex));
		assert(newChildIndex != nodeIndex && "Error - Node points at self");

		mData[nodeIndex][childId] = newChildIndex;
	}

	impl Default for NodeDAG{
		fn default() -> NodeStore {
			NodeStore{
				mEditNodesBegin: -1
			}
		}
	}

	impl NodeDAG{
		uint32 NodeDAG::countNodes(uint32 startNodeIndex) const
		{
			std::unordered_set<uint32> usedIndices;
			countNodes(startNodeIndex, usedIndices);
			return usedIndices.size();
		}

		// Counts the number of nodes at distinct locations (node indices).
		// A tree which is not fully merged will contain idetical nodes at different
		// locations in memory, and these will be counted seperately.
		void NodeDAG::countNodes(uint32 startNodeIndex, std::unordered_set<uint32>& usedIndices) const
		{
			// It may have been more efficient to have done this test before calling
			// into this function, but the implementation is simpler this way around.
			if (isMaterialNode(startNodeIndex)) { return; }

			usedIndices.insert(startNodeIndex);
			for (const uint32& childNodeIndex : mNodes[startNodeIndex])
			{
				countNodes(childNodeIndex, usedIndices);
			}
		}

		void NodeDAG::read(std::ifstream& file)
		{
			uint32_t nodeCount;
			file.read(reinterpret_cast<char*>(&nodeCount), sizeof(nodeCount));
			for (let mut ct: uint32 = 0; ct < nodeCount; ct++)
			{
				Node node;
				file.read(reinterpret_cast<char*>(&node), sizeof(node));
				mNodes.setNode(bakedNodesBegin() + ct, node);
			}
			mBakedNodesEnd = bakedNodesBegin() + nodeCount;
		}

		void NodeDAG::write(std::ofstream& file)
		{
			let mut nodeCount: uint32 = bakedNodesEnd() - bakedNodesBegin();
			file.write(reinterpret_cast<const char*>(&nodeCount), sizeof(nodeCount));
			for (let mut ct: uint32 = 0; ct < nodeCount; ct++)
			{
				file.write(reinterpret_cast<const char*>(&mNodes[bakedNodesBegin() + ct]), sizeof(Node));
			}
			
		}

		bool NodeDAG::isPrunable(const Node& node) const
		{
			if (!isMaterialNode(node[0])) { return false; }
			for (let mut i: uint32 = 1; i < 8; i++)
			{
				if (node[i] != node[0]) { return false; }
			}

			// All children represent the same solid material, so this node can be pruned.
			return true;
		}

		void NodeDAG::merge(uint32 index)
		{
			std::unordered_map<Node, uint32> map;
			let mut mergedEnd: uint32 = mEditNodesBegin;
			let mut nextSpace: uint32 = mergedEnd - 1;

			if (isMaterialNode(index))
			{
				mBakedNodesEnd = bakedNodesBegin();
			}
			else
			{
				let mut mergedRoot: uint32 = mergeNode(index, map, nextSpace);

				let mut actualNodeCount: uint32 = mergedEnd - mergedRoot;

				mBakedNodesEnd = bakedNodesBegin() + actualNodeCount;

				// FIXME - This offset value seems to get large. Is the logic backwards
				// but we ar wrapping around the array so it happens to work?
				let mut offset: uint32 = bakedNodesBegin() - mergedRoot;
				for (let mut nodeIndex: uint32 = bakedNodesBegin(); nodeIndex < bakedNodesEnd(); nodeIndex++)
				{
					let mut node: Node = mNodes[nodeIndex - offset];
					for (uint32& childIndex : node)
					{
						if (childIndex > MaxMaterial)
						{
							childIndex += offset;
						}
					}
					mNodes.setNode(nodeIndex, node);
				}
			}
		}

		uint32 NodeDAG::mergeNode(uint32 nodeIndex, std::unordered_map<Node, uint32>& map, uint32& nextSpace)
		{
			assert(!isMaterialNode(nodeIndex));
			const Node& oldNode = mNodes[nodeIndex];
			Node newNode;
			for (let mut i: int = 0; i < 8; i++)
			{
				let mut oldChildIndex: uint32 = oldNode[i];
				if (!isMaterialNode(oldChildIndex))
				{
					newNode[i] = mergeNode(oldChildIndex, map, nextSpace);
				}
				else
				{
					newNode[i] = oldNode[i];
				}
			}

			let mut newNodeIndex: uint32 = 0;
			let mut iter: auto = map.find(newNode);
			if (iter == map.end())
			{
				//newNodeIndex = insertNode(newNode)
				//newNodeIndex = mDAG.mInitialNodes.insert(newNode);

				mNodes.setNode(nextSpace, newNode);
				newNodeIndex = nextSpace;
				nextSpace--;

				map.insert({ newNode, newNodeIndex });
			}
			else
			{
				newNodeIndex = iter->second;
			}
			return newNodeIndex;
			//return nodes.insert(newNode);
		}

		uint32 NodeDAG::insert(const Node& node)
		{
			if (mEditNodesBegin > bakedNodesEnd())
			{
				mEditNodesBegin--;
				let mut index: uint32 = mEditNodesBegin;
				mNodes.setNode(index, node);
				return index;
			}

			throw std::runtime_error("Out of space for unshared edits!");
			return 0; // Indicates error (we don't use this function to allocate the zeroth node).
		}

		// Update the child of a node. If a copy needs to be made then this is returned,
		// otherwise the return value is empty to indicate that the update was done in-place.
		uint32 NodeDAG::updateNodeChild(uint32 nodeIndex, uint32 childId, uint32 newChildNodeIndex, bool forceCopy)
		{
			assert(newChildNodeIndex != mNodes[nodeIndex][childId]); // Watch for self-assignment (wasteful)
			assert(newChildNodeIndex != nodeIndex); // Don't let child point to parent.

			// Edit nodes can be modified in-place as they are unshared, unless the users
			// requests they are copied (e.g. for the purpose of maintaining an undo history).
			let mut modifyInPlace: const bool = isEditNode(nodeIndex) && (!forceCopy);

			if(modifyInPlace)
			{
				// Modify the existing node in-place by updating only the relevant child.
				mNodes.setNodeChild(nodeIndex, childId, newChildNodeIndex);

				// The modification may have made the node prunable. If so it must have taken on the value
				// of the new child so return that, otherwise return the updated node that we were given.
				return isPrunable(mNodes[nodeIndex]) ? newChildNodeIndex : nodeIndex;
			}
			else
			{
				let mut nodeIsMaterial: const bool = isMaterialNode(nodeIndex);

				// Make a copy of the existing node and then update the child.
				let mut newNode: Node = nodeIsMaterial ? makeNode(nodeIndex) : mNodes[nodeIndex];
				newNode[childId] = newChildNodeIndex;

				// If the copy becomes prunable as a result of the modification
				// then we can skip inserting it, which saves time and space.
				return isPrunable(newNode) ? newChildNodeIndex : insert(newNode);
			}
			
		}
	}

	////////////////////////////////////////////////////////////////////////////////
	// Public member functions
	////////////////////////////////////////////////////////////////////////////////

	impl Volume{
		Volume::Volume()
		{
			mRootNodeIndices.resize(1);
			mCurrentRoot = 0;
		}

		Volume::Volume(const std::string& filename)
		{
			mRootNodeIndices.resize(1);
			mCurrentRoot = 0;

			load(filename);
		}

		void Volume::fill(MaterialId matId)
		{
			setRootNodeIndex(matId);
		}

		uint32 Volume::rootNodeIndex() const
		{
			return mRootNodeIndices[mCurrentRoot];
		}

		void Volume::setRootNodeIndex(uint32 newRootNodeIndex)
		{
			if (mTrackEdits)
			{
				// When tracking edits a root node should not be set to itself. However, this
				// might happen when *not* tracking edits because we are then modifying in-place.
				assert(newRootNodeIndex != mRootNodeIndices[mCurrentRoot]);

				mCurrentRoot++;
				if (mCurrentRoot >= mRootNodeIndices.size())
				{
					mRootNodeIndices.resize(mCurrentRoot + 1);
				}
			}

			mRootNodeIndices[mCurrentRoot] = newRootNodeIndex;
		}

		void Volume::setTrackEdits(bool trackEdits)
		{
			mTrackEdits = trackEdits;
			if (!mTrackEdits)
			{
				mRootNodeIndices[0] = mRootNodeIndices[mCurrentRoot];
				mRootNodeIndices.resize(1);
				mCurrentRoot = 0;
			}
		}

		bool Volume::undo()
		{
			if (mCurrentRoot > 0)
			{
				mCurrentRoot--;
				return true;
			}

			return false; // Nothing to undo
		}

		bool Volume::redo()
		{
			if (mCurrentRoot < mRootNodeIndices.size() - 1)
			{
				mCurrentRoot++;
				return true;
			}

			return false; // Nothing to redo
		}

		void Volume::bake()
		{
			mDAG.merge(rootNodeIndex());
			setRootNodeIndex(mDAG.bakedNodesBegin());
		}

		void Volume::setVoxelRecursive(int32_t x, int32_t y, int32_t z, MaterialId matId)
		{
			// Do we need this mapping to unsiged space? Or could we eliminate it in both voxel()/
			//setVoxel() and get the same behaviour? Or is that confusing, e.g. with raycasting?
			let mut ux: uint32 = static_cast<uint32>(x) ^ (1UL << 31);
			let mut uy: uint32 = static_cast<uint32>(y) ^ (1UL << 31);
			let mut uz: uint32 = static_cast<uint32>(z) ^ (1UL << 31);

			let mut rootHeight: const int = logBase2(VolumeSideLength);
			let mut newRootNodeIndex: uint32 = setVoxelRecursive(ux, uy, uz, matId, rootNodeIndex(), rootHeight);
			setRootNodeIndex(newRootNodeIndex);
		}

		uint32 Volume::setVoxelRecursive(uint32 ux, uint32 uy, uint32 uz, MaterialId matId, uint32 nodeIndex, int nodeHeight)
		{
			assert(nodeHeight > 0);
			let mut childHeight: uint32_t = nodeHeight - 1;
			
			// We could possibly remove these variable bitshifts, but I'm not sure it's worth the effort.
			let mut childX: uint32_t = (ux >> childHeight) & 0x01;
			let mut childY: uint32_t = (uy >> childHeight) & 0x01;
			let mut childZ: uint32_t = (uz >> childHeight) & 0x01;
			let mut childId: uint32_t = childZ << 2 | childY << 1 | childX;

			let mut nodeIsMaterial: const bool = isMaterialNode(nodeIndex);

			// If current node is a material then just propergate it. Otherwise get the true child.
			let mut childNodeIndex: uint32_t = nodeIsMaterial ? nodeIndex : mDAG[nodeIndex][childId];

			// If the child node is set to the desired material then the voxel is 
			// already set. Return invalid node indicating nothing to update.
			if (childNodeIndex == matId) { return nodeIndex; }

			// Recusively process the child node until we reach a just-above-leaf node (height of 1).
			if (nodeHeight > 1)
			{
				let mut newChildNodeIndex: uint32 = setVoxelRecursive(ux, uy, uz, matId, childNodeIndex, nodeHeight - 1);

				// If the child hasn't changed then we don't need to update the current node.
				if (newChildNodeIndex == childNodeIndex) { return nodeIndex; }

				return mDAG.updateNodeChild(nodeIndex, childId, newChildNodeIndex, mTrackEdits);
			}
			else
			{
				return mDAG.updateNodeChild(nodeIndex, childId, matId, mTrackEdits);
			}
		}

		void Volume::setVoxel(int32_t x, int32_t y, int32_t z, MaterialId matId)
		{
			//return setVoxelRecursive(x, y, z, matId);

			struct NodeState
			{
				uint32_t mIndex;
				bool mProcessedNode;
			};

			// Note that the first two elements of this stack never actually get used.
			// Leaf and almost-leaf nodes(heights 0 and 1) never get put on the stack.
			// We accept this wasted space, rather than subtracting two on every access.
			let mut maxStackDepth: const int = 33;
			NodeState nodeStateStack[maxStackDepth];

			let mut rootHeight: const int = logBase2(VolumeSideLength);
			let mut nodeHeight: int = rootHeight;
			nodeStateStack[nodeHeight].mIndex = rootNodeIndex();
			nodeStateStack[nodeHeight].mProcessedNode = false;

			while (true)
			{
				// This loop does not go right down to leaf nodes, it stops one level above. But for any
				// given node it manipulates it's children, which means leaf nodes can get modified.
				assert(nodeHeight >= 1);

				NodeState& nodeState = nodeStateStack[nodeHeight];
				//let mut node: const Node* = &(mDAG[nodeState.mIndex]);

				// Find which subtree we are in.
				let mut childHeight: uint32_t = nodeHeight - 1;
				let mut tx: int = (x ^ (1UL << 31)); // Could precalculte these.
				let mut ty: int = (y ^ (1UL << 31));
				let mut tz: int = (z ^ (1UL << 31));
				let mut childX: uint32_t = (tx >> childHeight) & 0x01;
				let mut childY: uint32_t = (ty >> childHeight) & 0x01;
				let mut childZ: uint32_t = (tz >> childHeight) & 0x01;
				let mut childId: uint32_t = childZ << 2 | childY << 1 | childX;

				if (nodeState.mProcessedNode == false) // Executed the first time we see a node - i.e. as we move *down* the tree.
				{
					let mut nodeIsMaterial: const bool = isMaterialNode(nodeState.mIndex);

					// If current node is a material then just propergate it. Otherwise get the true child.
					let mut childNodeIndex: uint32_t = nodeIsMaterial ? nodeState.mIndex : mDAG[nodeState.mIndex][childId];

					// If the child node is set to the desired material then the voxel is already set. 
					if (childNodeIndex == matId) { return; }

					if (nodeHeight >= 2)
					{
						NodeState& childNodeState = nodeStateStack[nodeHeight - 1];
						childNodeState.mIndex = childNodeIndex;
						childNodeState.mProcessedNode = false;

						nodeHeight -= 1;
					}

					nodeState.mProcessedNode = true;
				}
				else // Executed the second time we see a node - i.e. as we move *up* the tree.
				{
					if (nodeHeight > 1)
					{
						// If the child has changed then we need to update the current node.
						const NodeState& childNodeState = nodeStateStack[nodeHeight - 1];
						if (mDAG[nodeState.mIndex][childId] != childNodeState.mIndex)
						{
							nodeState.mIndex = mDAG.updateNodeChild(nodeState.mIndex, childId, childNodeState.mIndex, mTrackEdits);
						}
					}
					else
					{
						nodeState.mIndex = mDAG.updateNodeChild(nodeState.mIndex, childId, matId, mTrackEdits);
					}

					// Move up the tree to process parent node next, until we reach the root.
					nodeHeight += 1;
					if (nodeHeight > rootHeight)
					{
						break;
					}
				}
			}

			setRootNodeIndex(nodeStateStack[rootHeight].mIndex);
		}

		void Volume::fillBrush(const Brush& brush, MaterialId matId)
		{
			let mut rootHeight: const int = logBase2(VolumeSideLength);
			let mut nodeHeight: int = rootHeight;
			let mut newIndex: uint32_t = matId;

			let mut rootLowerBound: constexpr int32 = std::numeric_limits<int32>::min();

			let mut newRootNodeIndex: uint32 = fillBrush(brush, matId, rootNodeIndex(), nodeHeight, rootLowerBound, rootLowerBound, rootLowerBound);
			setRootNodeIndex(newRootNodeIndex);
		}

		uint32 Volume::fillBrush(const Brush& brush, MaterialId matId, uint32 nodeIndex, int nodeHeight, int32 nodeLowerX, int32 nodeLowerY, int32 nodeLowerZ)
		{
			let mut childHeight: uint32_t = nodeHeight - 1;
			//let mut tx: int = (x ^ (1UL << 31)); // Could precalculte these.
			//let mut ty: int = (y ^ (1UL << 31));
			//let mut tz: int = (z ^ (1UL << 31));

			for (let mut childZ: uint32 = 0; childZ <= 1; childZ++)
			{
				for (let mut childY: uint32 = 0; childY <= 1; childY++)
				{
					for (let mut childX: uint32 = 0; childX <= 1; childX++)
					{
						//let mut childX: uint32_t = (tx >> childHeight) & 0x01;
						//let mut childY: uint32_t = (ty >> childHeight) & 0x01;
						//let mut childZ: uint32_t = (tz >> childHeight) & 0x01;
						let mut childId: uint32_t = childZ << 2 | childY << 1 | childX;

						let mut childSideLength: uint32_t = 1 << (childHeight);
						let mut childLowerX: int32 = nodeLowerX + (childSideLength * childX);
						let mut childLowerY: int32 = nodeLowerY + (childSideLength * childY);
						let mut childLowerZ: int32 = nodeLowerZ + (childSideLength * childZ);

						let mut childUpperX: int32 = childLowerX + (childSideLength - 1);
						let mut childUpperY: int32 = childLowerY + (childSideLength - 1);
						let mut childUpperZ: int32 = childLowerZ + (childSideLength - 1);

						Box3f childBounds(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childLowerY), static_cast<float>(childLowerZ) }), Vector3f({ static_cast<float>(childUpperX),static_cast<float>(childUpperY), static_cast<float>(childUpperZ) }));

						if (!overlaps(brush.bounds(), childBounds))
						{
							continue;
						}

						let mut allCornersInsideBrush: bool = true;
						if (!brush.contains(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childLowerY), static_cast<float>(childLowerZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childLowerY), static_cast<float>(childUpperZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childUpperY), static_cast<float>(childLowerZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childUpperY), static_cast<float>(childUpperZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childUpperX), static_cast<float>(childLowerY), static_cast<float>(childLowerZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childUpperX), static_cast<float>(childLowerY), static_cast<float>(childUpperZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childUpperX), static_cast<float>(childUpperY), static_cast<float>(childLowerZ) }))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f({ static_cast<float>(childUpperX), static_cast<float>(childUpperY), static_cast<float>(childUpperZ) }))) { allCornersInsideBrush = false; }

						let mut nodeIsMaterial: const bool = isMaterialNode(nodeIndex);

						// If current node is a material then just propergate it. Otherwise get the true child.
						let mut childNodeIndex: uint32_t = nodeIsMaterial ? nodeIndex : mDAG[nodeIndex][childId];

						// If the child node is set to the desired material then the voxel is already set.
						if (childNodeIndex == matId) { continue; }

						// Process children
						let mut newChildNodeIndex: uint32 = childNodeIndex;
						//if (nodeHeight >= 2)
						if(nodeHeight >= 2 && !allCornersInsideBrush)
						{
							newChildNodeIndex = fillBrush(brush, matId, childNodeIndex, nodeHeight - 1, childLowerX, childLowerY, childLowerZ);
						}
						else
						{
							if (allCornersInsideBrush)
							{
								newChildNodeIndex = matId;
							}
						}

						// If the child has changed then we need to update the current node.
						if (childNodeIndex != newChildNodeIndex)
						{
							nodeIndex = mDAG.updateNodeChild(nodeIndex, childId, newChildNodeIndex, mTrackEdits);
						}
					}
				}
			}

			return nodeIndex;
		}

		void Volume::addVolume(const Volume& rhsVolume)
		{
			let mut rootHeight: const int = logBase2(VolumeSideLength);
			let mut nodeHeight: int = rootHeight;

			let mut rootLowerBound: constexpr int32 = std::numeric_limits<int32>::min();

			let mut newRootNodeIndex: uint32 = addVolume(rhsVolume, rhsVolume.rootNodeIndex(), rootNodeIndex(), nodeHeight, rootLowerBound, rootLowerBound, rootLowerBound);
			setRootNodeIndex(newRootNodeIndex);
		}

		// Fixme - This function can prbably be more efficient. Firstly through better early out when whole nodes are full/empty, and also
		// if seperate volumes shared a common memory space it would be easier to copy node directly across (maybe in compressd space?).
		uint32 Volume::addVolume(const Volume& rhsVolume, uint32 rhsNodeIndex, uint32 nodeIndex, int nodeHeight, int32 nodeLowerX, int32 nodeLowerY, int32 nodeLowerZ)
		{
			let mut childHeight: uint32_t = nodeHeight - 1;
			//let mut tx: int = (x ^ (1UL << 31)); // Could precalculte these.
			//let mut ty: int = (y ^ (1UL << 31));
			//let mut tz: int = (z ^ (1UL << 31));

			for (let mut childZ: uint32 = 0; childZ <= 1; childZ++)
			{
				for (let mut childY: uint32 = 0; childY <= 1; childY++)
				{
					for (let mut childX: uint32 = 0; childX <= 1; childX++)
					{
						//let mut childX: uint32_t = (tx >> childHeight) & 0x01;
						//let mut childY: uint32_t = (ty >> childHeight) & 0x01;
						//let mut childZ: uint32_t = (tz >> childHeight) & 0x01;
						let mut childId: uint32_t = childZ << 2 | childY << 1 | childX;

						let mut childSideLength: uint32_t = 1 << (childHeight);
						let mut childLowerX: int32 = nodeLowerX + (childSideLength * childX);
						let mut childLowerY: int32 = nodeLowerY + (childSideLength * childY);
						let mut childLowerZ: int32 = nodeLowerZ + (childSideLength * childZ);

						let mut childUpperX: int32 = childLowerX + (childSideLength - 1);
						let mut childUpperY: int32 = childLowerY + (childSideLength - 1);
						let mut childUpperZ: int32 = childLowerZ + (childSideLength - 1);

						Box3f childBounds(Vector3f({ static_cast<float>(childLowerX), static_cast<float>(childLowerY), static_cast<float>(childLowerZ) } ), Vector3f({ static_cast<float>(childUpperX), static_cast<float>(childUpperY), static_cast<float>(childUpperZ) }));

						/*if (!overlaps(brush.bounds(), childBounds))
						{
							continue;
						}*/

						/let mut allCornersInsideBrush: *bool = true;
						if (!brush.contains(Vector3f(childLowerX, childLowerY, childLowerZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childLowerX, childLowerY, childUpperZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childLowerX, childUpperY, childLowerZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childLowerX, childUpperY, childUpperZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childUpperX, childLowerY, childLowerZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childUpperX, childLowerY, childUpperZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childUpperX, childUpperY, childLowerZ))) { allCornersInsideBrush = false; }
						if (!brush.contains(Vector3f(childUpperX, childUpperY, childUpperZ))) { allCornersInsideBrush = false; }*/

						let mut nodeIsMaterial: const bool = isMaterialNode(nodeIndex);
						let mut rhsNodeIsMaterial: const bool = isMaterialNode(rhsNodeIndex);

						// If current node is a material then just propergate it. Otherwise get the true child.
						let mut childNodeIndex: uint32_t = nodeIsMaterial ? nodeIndex : mDAG[nodeIndex][childId];
						let mut rhsChildNodeIndex: uint32_t = rhsNodeIsMaterial ? rhsNodeIndex : rhsVolume.mDAG[rhsNodeIndex][childId];

						// If the child node is set to the desired material then the voxel is already set.
						if (isMaterialNode(childNodeIndex) && isMaterialNode(rhsChildNodeIndex) && (childNodeIndex == rhsChildNodeIndex)) { continue; }

						// Process children
						let mut newChildNodeIndex: uint32 = childNodeIndex;

						// We have to treat at least one material as empty space, otherwise we are just copying 
						// every voxel from source to destination, which just result in a copy of the source. 
						// Hard-code it to zero for now, but should think about how else it might be controlled.
						let mut emptyMaterial: MaterialId = 0;
						if (rhsChildNodeIndex != emptyMaterial)
						{
							if (isMaterialNode(rhsChildNodeIndex))
							{
								newChildNodeIndex = rhsChildNodeIndex;
							}
							else
							{
								newChildNodeIndex = addVolume(rhsVolume, rhsChildNodeIndex, childNodeIndex, nodeHeight - 1, childLowerX, childLowerY, childLowerZ);;
							}
						}

						// If the child has changed then we need to update the current node.
						if (childNodeIndex != newChildNodeIndex)
						{
							nodeIndex = mDAG.updateNodeChild(nodeIndex, childId, newChildNodeIndex, mTrackEdits);
						}
					}
				}
			}

			return nodeIndex;
		}

		MaterialId Volume::voxel(int32_t x, int32_t y, int32_t z) const
		{
			let mut nodeIndex: uint32_t = rootNodeIndex();
			let mut height: uint32_t = logBase2(VolumeSideLength);

			// FIXME - think whether we need the line below - I think we do for empty/solid volumes?
			//if (mDAG.isMaterialNode(mRootNodeIndex)) { return static_cast<MaterialId>(mRootNodeIndex); }

			while (height >= 1)
			{			
				// If we reach a full node then the requested voxel is occupied.
				if (isMaterialNode(nodeIndex)) { return static_cast<MaterialId>(nodeIndex); }

				// Otherwise find which subtree we are in.
				// Optimization - Note that the code below requires shifting by a variable amount which can be slow.
				// Alternatively I think we can simply shift x, y, and z by one bit per iteration, but this requires us
				// to reverse the order of the bits at the start of this function. It would be a one-time cost for a
				// faster loop, and testing is needed (on a real, large volume) to determine whether it is beneficial.
				let mut childHeight: uint32_t = height - 1;
				let mut tx: int = (x ^ (1UL << 31)); // Could precalculte these.
				let mut ty: int = (y ^ (1UL << 31));
				let mut tz: int = (z ^ (1UL << 31));
				let mut childX: uint32_t = (tx >> childHeight) & 0x01;
				let mut childY: uint32_t = (ty >> childHeight) & 0x01;
				let mut childZ: uint32_t = (tz >> childHeight) & 0x01;
				let mut childId: uint32_t = childZ << 2 | childY << 1 | childX;

				// Prepare for next iteration.
				nodeIndex = mDAG[nodeIndex][childId];
				height--;
			}

			// We have reached a height of zero so the node must be a material node.
			assert(height == 0 && isMaterialNode(nodeIndex));
			return static_cast<MaterialId>(nodeIndex);
		}

		////////////////////////////////////////////////////////////////////////////////
		// Private member functions
		////////////////////////////////////////////////////////////////////////////////

		bool Volume::load(const std::string& filename)
		{
			std::ifstream file(filename, std::ios::binary);

			// FIXME - What should we do for error handling in this function? Return codes or exceptions?
			if(!file.is_open())
			{
				return false;
			}

			uint32 rootNodeIndex;
			file.read(reinterpret_cast<char*>(&rootNodeIndex), sizeof(rootNodeIndex));
			//assert(rootNodeIndex == mDAG.arrayBegin());
			//setRootNodeIndex(RefCountedNodeIndex(rootNodeIndex, &mDAG));

			//mDAG.read(file);

			// This fill is not required but can be useful for debugging
			//let mut InvalidNode: const Node = makeNode(0xffffffff);
			//std::fill(mDAG.mNodes.begin(), mDAG.mNodes.end(), InvalidNode);

			mDAG.read(file);

			if (rootNodeIndex >= MaterialCount)
			{
				rootNodeIndex += mDAG.bakedNodesBegin() - MaterialCount;
			}

			setRootNodeIndex(rootNodeIndex);

			return true;
		}

		void Volume::save(const std::string& filename)
		{
			bake();

			let mut root: uint32 = rootNodeIndex();
			const Node& data = mDAG[mDAG.bakedNodesBegin()];

			std::ofstream file;
			file = std::ofstream(filename, std::ios::out | std::ios::binary);
			file.write(reinterpret_cast<const char*>(&root), sizeof(root));
			mDAG.write(file);
			file.close();
		}
	}

	mod Internals
	{
		/// This is an advanced function which should only be used if you
		/// understand the internal memory layout of Cubiquity's volume data.
		NodeDAG& getNodes(Volume& volume)
		{
			return volume.mDAG;
		}

		const NodeDAG& getNodes(const Volume& volume)
		{
			return volume.mDAG;
		}

		const uint32 getRootNodeIndex(const Volume& volume)
		{
			return volume.rootNodeIndex();
		}
	}
}