use object_tree::{Hash, HashedNode};
use petgraph::prelude::*;

use super::{PkgResult, SiPkgError, Source};

use crate::{node::PkgNode, PositionSpec};

#[derive(Clone, Debug)]
pub struct SiPkgPosition<'a> {
    x: String,
    y: String,
    height: String,
    width: String,

    hash: Hash,
    source: Source<'a>,
}

impl<'a> SiPkgPosition<'a> {
    pub fn from_graph(
        graph: &'a Graph<HashedNode<PkgNode>, ()>,
        node_idx: NodeIndex,
    ) -> PkgResult<Self> {
        let hashed_node = &graph[node_idx];
        let node = match hashed_node.inner() {
            PkgNode::Position(node) => node.clone(),
            unexpected => {
                return Err(SiPkgError::UnexpectedPkgNodeType(
                    PkgNode::POSTITION_KIND_STR,
                    unexpected.node_kind_str(),
                ))
            }
        };

        Ok(Self {
            x: node.x,
            y: node.y,
            height: node.height,
            width: node.width,

            hash: hashed_node.hash(),
            source: Source::new(graph, node_idx),
        })
    }

    pub fn x(&self) -> &str {
        self.x.as_str()
    }

    pub fn y(&self) -> &str {
        self.y.as_str()
    }

    pub fn height(&self) -> &str {
        self.height.as_str()
    }

    pub fn width(&self) -> &str {
        self.width.as_str()
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn source(&self) -> &Source<'a> {
        &self.source
    }
}

impl<'a> TryFrom<SiPkgPosition<'a>> for PositionSpec {
    type Error = SiPkgError;

    fn try_from(value: SiPkgPosition<'a>) -> Result<Self, Self::Error> {
        Ok(PositionSpec::builder()
            .x(value.x())
            .y(value.y())
            .height(value.height())
            .width(value.width())
            .build()?)
    }
}
