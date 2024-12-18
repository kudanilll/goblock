#ifndef CHUNK_H
#define CHUNK_H

#include <vector>

#include "../block/block.h"

class Chunk {
 public:
  Chunk(glm::vec3 chunk_position);

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;

  std::vector<Block> get_blocks() const;

 private:
  glm::vec3 position_;
  std::vector<Block> blocks_;
  static constexpr int CHUNK_SIZE = 16;
};

#endif
