#include "chunk.h"

#include <GL/glew.h>

#include <glm/gtc/matrix_transform.hpp>
#include <unordered_map>

namespace {

struct Vertex {
  glm::vec3 pos;
  glm::vec3 normal;
  glm::vec3 color;
};

const glm::vec3 kTopColor(0.20f, 0.60f, 0.20f);
const glm::vec3 kSideColor(0.45f, 0.28f, 0.15f);
const glm::vec3 kBottomColor(0.25f, 0.16f, 0.09f);

// Corners are in block-local space, block occupies [0,1] on each axis.
// Each face lists 4 corners counter-clockwise seen from outside.
void append_face(std::vector<Vertex>& out, const glm::vec3& origin,
                 const glm::vec3& normal, const glm::vec3& color,
                 const glm::vec3 (&corners)[4]) {
  Vertex a{origin + corners[0], normal, color};
  Vertex b{origin + corners[1], normal, color};
  Vertex c{origin + corners[2], normal, color};
  Vertex d{origin + corners[3], normal, color};
  out.insert(out.end(), {a, b, c, a, c, d});
}

}  // namespace

Chunk::Chunk(glm::vec3 chunk_position) : position_(chunk_position) {
  for (int x = 0; x < CHUNK_SIZE; ++x) {
    for (int z = 0; z < CHUNK_SIZE; ++z) {
      blocks_.emplace_back(
          glm::vec3(position_.x + x, position_.y, position_.z + z));
    }
  }
  build_mesh();
}

Chunk::~Chunk() { release_mesh(); }

Chunk::Chunk(const Chunk& other)
    : position_(other.position_), blocks_(other.blocks_) {
  build_mesh();
}

Chunk& Chunk::operator=(const Chunk& other) {
  if (this == &other) return *this;
  release_mesh();
  position_ = other.position_;
  blocks_ = other.blocks_;
  build_mesh();
  return *this;
}

void Chunk::build_mesh() {
  std::unordered_map<long long, bool> occupied;
  auto key = [](int x, int y, int z) {
    return (static_cast<long long>(x) << 32) ^
           (static_cast<long long>(z) << 16) ^ static_cast<long long>(y);
  };

  for (const auto& block : blocks_) {
    if (!block.is_active()) continue;
    glm::vec3 p = block.get_position();
    occupied[key(static_cast<int>(p.x) - static_cast<int>(position_.x),
                 static_cast<int>(p.y),
                 static_cast<int>(p.z) - static_cast<int>(position_.z))] = true;
  }

  const glm::vec3 top[4] = {{0, 1, 1}, {1, 1, 1}, {1, 1, 0}, {0, 1, 0}};
  const glm::vec3 bottom[4] = {{0, 0, 0}, {1, 0, 0}, {1, 0, 1}, {0, 0, 1}};
  const glm::vec3 front[4] = {{0, 0, 1}, {1, 0, 1}, {1, 1, 1}, {0, 1, 1}};
  const glm::vec3 back[4] = {{1, 0, 0}, {0, 0, 0}, {0, 1, 0}, {1, 1, 0}};
  const glm::vec3 left[4] = {{0, 0, 0}, {0, 0, 1}, {0, 1, 1}, {0, 1, 0}};
  const glm::vec3 right[4] = {{1, 0, 1}, {1, 0, 0}, {1, 1, 0}, {1, 1, 1}};

  std::vector<Vertex> vertices;
  vertices.reserve(blocks_.size() * 6 * 6);

  auto solid = [&](int x, int y, int z) {
    auto it = occupied.find(key(x, y, z));
    return it != occupied.end() && it->second;
  };

  for (const auto& block : blocks_) {
    if (!block.is_active()) continue;
    glm::vec3 p = block.get_position();
    int lx = static_cast<int>(p.x) - static_cast<int>(position_.x);
    int ly = static_cast<int>(p.y);
    int lz = static_cast<int>(p.z) - static_cast<int>(position_.z);

    // Skip faces shared with a neighbouring block; they are never visible.
    if (!solid(lx, ly + 1, lz))
      append_face(vertices, p, {0, 1, 0}, kTopColor, top);
    if (!solid(lx, ly - 1, lz))
      append_face(vertices, p, {0, -1, 0}, kBottomColor, bottom);
    if (!solid(lx, ly, lz + 1))
      append_face(vertices, p, {0, 0, 1}, kSideColor, front);
    if (!solid(lx, ly, lz - 1))
      append_face(vertices, p, {0, 0, -1}, kSideColor, back);
    if (!solid(lx - 1, ly, lz))
      append_face(vertices, p, {-1, 0, 0}, kSideColor, left);
    if (!solid(lx + 1, ly, lz))
      append_face(vertices, p, {1, 0, 0}, kSideColor, right);
  }

  vertex_count_ = static_cast<int>(vertices.size());

  if (vertex_count_ == 0) return;

  glGenVertexArrays(1, &vao_);
  glGenBuffers(1, &vbo_);
  glBindVertexArray(vao_);
  glBindBuffer(GL_ARRAY_BUFFER, vbo_);
  glBufferData(GL_ARRAY_BUFFER,
               static_cast<GLsizeiptr>(vertices.size() * sizeof(Vertex)),
               vertices.data(), GL_STATIC_DRAW);

  glEnableVertexAttribArray(0);
  glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex),
                        (void*)offsetof(Vertex, pos));
  glEnableVertexAttribArray(1);
  glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex),
                        (void*)offsetof(Vertex, normal));
  glEnableVertexAttribArray(2);
  glVertexAttribPointer(2, 3, GL_FLOAT, GL_FALSE, sizeof(Vertex),
                        (void*)offsetof(Vertex, color));

  glBindVertexArray(0);
}

void Chunk::release_mesh() {
  if (vbo_) {
    glDeleteBuffers(1, &vbo_);
    vbo_ = 0;
  }
  if (vao_) {
    glDeleteVertexArrays(1, &vao_);
    vao_ = 0;
  }
  vertex_count_ = 0;
}

void Chunk::draw(const glm::mat4&, const glm::mat4&) const {
  if (!vao_ || vertex_count_ == 0) return;
  glBindVertexArray(vao_);
  glDrawArrays(GL_TRIANGLES, 0, vertex_count_);
  glBindVertexArray(0);
}

std::vector<Block> Chunk::get_blocks() const { return blocks_; }

glm::vec3 Chunk::get_min() const { return position_; }

glm::vec3 Chunk::get_max() const {
  return position_ + glm::vec3(CHUNK_SIZE, 1.0f, CHUNK_SIZE);
}
