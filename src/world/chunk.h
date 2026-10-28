#ifndef CHUNK_H
#define CHUNK_H

#include <glm/glm.hpp>
#include <vector>

#include "../block/block.h"

class Chunk {
 public:
  Chunk(glm::vec3 chunk_position);
  ~Chunk();

  Chunk(const Chunk& other);
  Chunk& operator=(const Chunk& other);

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;

  std::vector<Block> get_blocks() const;

  glm::vec3 get_min() const;
  glm::vec3 get_max() const;

  static constexpr int CHUNK_SIZE = 16;

 private:
  glm::vec3 position_;
  std::vector<Block> blocks_;

  unsigned int vao_ = 0;
  unsigned int vbo_ = 0;
  int vertex_count_ = 0;

  void build_mesh();
  void release_mesh();
};

#endif
