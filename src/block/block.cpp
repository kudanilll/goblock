#include "block.h"

Block::Block(glm::vec3 position, bool active, bool has_collision)
    : position_(position), active_(active), has_collision_(has_collision) {}

bool Block::is_point_inside_block(const glm::vec3& point) const {
  float half_size = 0.5f;  // Ukuran setengah dari sisi blok
  return (point.x >= position_.x - half_size &&
          point.x <= position_.x + half_size &&
          point.y >= position_.y - half_size &&
          point.y <= position_.y + half_size &&
          point.z >= position_.z - half_size &&
          point.z <= position_.z + half_size);
}

bool Block::is_active() const { return active_; }

bool Block::has_collision() const { return has_collision_; }

glm::vec3 Block::get_position() const { return position_; }
