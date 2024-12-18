#ifndef BLOCK_H
#define BLOCK_H

#include <glm/glm.hpp>

class Block {
 public:
  Block(glm::vec3 position, bool active = true, bool has_collision = true);

  void draw(const glm::mat4& view_matrix,
            const glm::mat4& projection_matrix) const;

  bool is_point_inside_block(const glm::vec3& point) const;
  bool is_active() const;
  bool has_collision() const;
  glm::vec3 get_position() const;

 private:
  glm::vec3 position_;
  bool active_;
  bool has_collision_;
};

#endif
