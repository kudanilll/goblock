#include "block/block.h"

#include <GL/glew.h>

#include <glm/gtc/matrix_transform.hpp>

Block::Block(glm::vec3 position, bool active)
    : position_(position), active_(active) {}

bool Block::is_active() const { return active_; }

void Block::draw(const glm::mat4& view_matrix,
                 const glm::mat4& projection_matrix) const {
  if (!active_) return;

  glm::mat4 model_matrix = glm::translate(glm::mat4(1.0f), position_);
  glm::mat4 mvp_matrix = projection_matrix * view_matrix * model_matrix;

  // Mengatur warna hijau untuk blok
  glMatrixMode(GL_MODELVIEW);
  glLoadMatrixf(&mvp_matrix[0][0]);

  glBegin(GL_QUADS);
  glColor3f(0.0f, 0.6f, 0.0f);  // Warna hijau kustom

  // Face atas
  glVertex3f(0, 0, 0);
  glVertex3f(1, 0, 0);
  glVertex3f(1, 0, 1);
  glVertex3f(0, 0, 1);

  // Face bawah
  glVertex3f(0, -1, 0);
  glVertex3f(1, -1, 0);
  glVertex3f(1, -1, 1);
  glVertex3f(0, -1, 1);

  glEnd();
}