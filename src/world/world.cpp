#include "world.h"

World::World() {
  for (int x = 0; x < WORLD_SIZE; ++x) {
    for (int z = 0; z < WORLD_SIZE; ++z) {
      chunks_.emplace_back(glm::vec3(x * 16, 0, z * 16));
    }
  }
}

void World::draw(const glm::mat4& view_matrix,
                 const glm::mat4& projection_matrix) const {
  for (const auto& chunk : chunks_) {
    chunk.draw(view_matrix, projection_matrix);
  }
}

std::vector<Chunk>& World::get_chunks() { return chunks_; }
