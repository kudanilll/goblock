#include "utils.h"

#include <GL/glut.h>
#include <GLFW/glfw3.h>

void draw_text(const std::string& text, float x, float y) {
  glMatrixMode(GL_PROJECTION);
  glPushMatrix();
  glLoadIdentity();
  glOrtho(0, 800, 0, 600, -1, 1);

  glMatrixMode(GL_MODELVIEW);
  glPushMatrix();
  glLoadIdentity();

  glRasterPos2f(x, y);  // Koordinat teks

  for (char c : text) {
    glutBitmapCharacter(GLUT_BITMAP_9_BY_15, c);  // Font bawaan OpenGL
  }

  glMatrixMode(GL_MODELVIEW);
  glPopMatrix();

  glMatrixMode(GL_PROJECTION);
  glPopMatrix();
}