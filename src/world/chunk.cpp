#include "chunk.h"

#include <glm/gtc/matrix_transform.hpp>

Chunk::Chunk(glm::vec3 chunk_position) : position_(chunk_position) {
  // Inisialisasi block dalam chunk
  for (int x = 0; x < CHUNK_SIZE; ++x) {
    for (int z = 0; z < CHUNK_SIZE; ++z) {
      blocks_.emplace_back(
          glm::vec3(position_.x + x, position_.y, position_.z + z));
    }
  }
}

void Chunk::draw(const glm::mat4& view_matrix,
                 const glm::mat4& projection_matrix) const {
  for (const auto& block : blocks_) {
    block.draw(view_matrix, projection_matrix);
  }
}
