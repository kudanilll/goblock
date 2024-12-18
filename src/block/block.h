#ifndef BLOCK_H
#define BLOCK_H

#include <glm/glm.hpp>

class Block {
 public:
  Block(glm::vec3 position, bool active = true);

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;
  bool is_active() const;

 private:
  glm::vec3 position_;
  bool active_;
};

#endif
