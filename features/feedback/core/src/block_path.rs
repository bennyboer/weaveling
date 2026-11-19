/// Where a block lies in its scene, as the indices that lead from the scene down to it.
/// The first index is the block's place among the scene's blocks.
/// Inside a quote, the next index is the block's place among the quote's blocks.
/// Inside a list, the next two indices are the item's place in the list and the block's place in that item.
/// So the second paragraph of a list's first item, the list being the scene's third block, lies at `[2, 0, 1]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockPath(Vec<u32>);

impl BlockPath {
    pub fn new(indices: Vec<u32>) -> Self {
        Self(indices)
    }

    pub fn indices(&self) -> &[u32] {
        &self.0
    }

    /// Whether it leads to no block at all.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
