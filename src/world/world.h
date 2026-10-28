#ifndef WORLD_H
#define WORLD_H

#include <glm/glm.hpp>
#include <vector>

#include "chunk.h"

class World {
 public:
  World();

  World(const World&) = delete;
  World& operator=(const World&) = delete;

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;
  std::vector<Chunk>& get_chunks();

  static constexpr int WORLD_SIZE = 8;

 private:
  std::vector<Chunk> chunks_;

  // Returns the 6 clip-space planes of projection * view.
  static void extract_frustum(const glm::mat4& vp, glm::vec4 planes[6]);
  static bool box_in_frustum(const glm::vec4 planes[6], const glm::vec3& min,
                             const glm::vec3& max);
};

#endif
