#include "world.h"

#include "chunk.h"

World::World() {
  chunks_.reserve(WORLD_SIZE * WORLD_SIZE);
  for (int x = 0; x < WORLD_SIZE; ++x) {
    for (int z = 0; z < WORLD_SIZE; ++z) {
      chunks_.emplace_back(
          glm::vec3(x * Chunk::CHUNK_SIZE, 0, z * Chunk::CHUNK_SIZE));
    }
  }
}

void World::extract_frustum(const glm::mat4& vp, glm::vec4 planes[6]) {
  // Gribb-Hartmann: each plane is a sum/difference of the matrix rows.
  const glm::vec4 r0 = vp[0];
  const glm::vec4 r1 = vp[1];
  const glm::vec4 r2 = vp[2];
  const glm::vec4 r3 = vp[3];
  planes[0] = r3 + r0;  // left
  planes[1] = r3 - r0;  // right
  planes[2] = r3 + r1;  // bottom
  planes[3] = r3 - r1;  // top
  planes[4] = r3 + r2;  // near
  planes[5] = r3 - r2;  // far

  for (int i = 0; i < 6; ++i) {
    float len = glm::length(glm::vec3(planes[i]));
    if (len > 0.0f) planes[i] /= len;
  }
}

bool World::box_in_frustum(const glm::vec4 planes[6], const glm::vec3& min,
                           const glm::vec3& max) {
  for (int i = 0; i < 6; ++i) {
    glm::vec3 p;
    // Pick the box corner furthest along the plane normal.
    p.x = planes[i].x > 0.0f ? max.x : min.x;
    p.y = planes[i].y > 0.0f ? max.y : min.y;
    p.z = planes[i].z > 0.0f ? max.z : min.z;
    if (glm::dot(glm::vec3(planes[i]), p) + planes[i].w < 0.0f) {
      return false;
    }
  }
  return true;
}

void World::draw(const glm::mat4& view_matrix,
                 const glm::mat4& projection_matrix) const {
  glm::vec4 planes[6];
  extract_frustum(projection_matrix * view_matrix, planes);

  for (const auto& chunk : chunks_) {
    if (box_in_frustum(planes, chunk.get_min(), chunk.get_max())) {
      chunk.draw(view_matrix, projection_matrix);
    }
  }
}

std::vector<Chunk>& World::get_chunks() { return chunks_; }
