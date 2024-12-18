#include "utils.h"

#include <GL/glew.h>
#include <GL/glut.h>
#include <GLFW/glfw3.h>
#include <stdio.h>

#include <iostream>

void utils::draw_text(const std::string& text, float x, float y) {
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

void utils::show_fps(double& delta_time, double& last_time, unsigned int& loops,
                     char* fps_buff, size_t buffer_size) {
  double current_time = glfwGetTime();    // Waktu saat ini
  delta_time = current_time - last_time;  // Hitung delta_time
  loops++;

  if (delta_time >= 1.0) {  // Update setiap 1 detik
    double fps = loops / delta_time;
    snprintf(fps_buff, buffer_size, "FPS: %d",
             static_cast<int>(fps));  // Format FPS ke buffer
    std::cout << "Current FPS: " << static_cast<int>(fps) << std::endl;
    loops = 0;                 // Reset counter loop
    last_time = current_time;  // Reset waktu terakhir
  }

  glColor3f(1.0f, 0.0f, 0.0f);
  draw_text(fps_buff, 10.0f, 10.0f);
}
