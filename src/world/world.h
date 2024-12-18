#ifndef WORLD_H
#define WORLD_H

#include <vector>

#include "chunk.h"

class World {
 public:
  World();

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;

 private:
  std::vector<Chunk> chunks_;
  static constexpr int WORLD_SIZE = 8;
};

#endif
