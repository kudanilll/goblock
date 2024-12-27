#include <GL/glew.h>
#include <GL/glut.h>
#include <GLFW/glfw3.h>
#include <stdio.h>

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <iostream>

#include "control/camera.h"
#include "control/control.h"
#include "utils/utils.h"
#include "world/world.h"

char player_pos_buff[0xFF];
char fps_buff[0xFF];

// FPS variable
double last_time = 0.0;
int frame_count = 0;
float fps = 0.0f;

void update_fps() {
  double current_time = glfwGetTime();
  frame_count++;
  // Hitung FPS setiap detik
  if (current_time - last_time >= 1.0) {
    fps = frame_count / (current_time - last_time);
    frame_count = 0;
    last_time = current_time;
  }
}

// Init OpenGL dan GLFW
GLFWwindow* initialize_window(int width, int height, const char* title) {
  if (!glfwInit()) {
    return nullptr;
  }

  GLFWwindow* window = glfwCreateWindow(width, height, title, nullptr, nullptr);
  if (!window) {
    glfwTerminate();
    return nullptr;
  }

  glfwMakeContextCurrent(window);
  glewExperimental = GL_TRUE;
  if (glewInit() != GLEW_OK) {
    glfwTerminate();
    return nullptr;
  }

  glEnable(GL_DEPTH_TEST);
  glDepthFunc(GL_LESS);
  return window;
}

int main(int argc, char** argv) {
  const int window_width = 800;
  const int window_height = 600;

  glutInit(&argc, argv);

  GLFWwindow* window =
      initialize_window(window_width, window_height, "goblock");
  if (!window) return -1;

  // World
  World world;
  std::vector<Chunk>& chunks = world.get_chunks();
  std::vector<Block> blocks;
  float block_size = 1.0f;

  for (auto& chunk : chunks) {
    const std::vector<Block>& chunk_blocks = chunk.get_blocks();
    blocks.insert(blocks.end(), chunk_blocks.begin(), chunk_blocks.end());
  }

  // Kamera & Kontrol
  Camera camera;
  Control control(window, camera, blocks, block_size);

  // Init last time
  last_time = glfwGetTime();

  while (!glfwWindowShouldClose(window)) {
    // Update FPS
    update_fps();

    // Input & Update
    control.handle_input(blocks);
    camera.update();

    // Render
    glClearColor(0.53f, 0.81f, 0.98f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    world.draw(camera.get_view_matrix(), camera.get_projection_matrix());

    // Player position
    snprintf(player_pos_buff, sizeof(player_pos_buff),
             "Player Position: (%f, %f, %f)", camera.position.x,
             camera.position.y, camera.position.z);
    glColor3f(1.0f, 1.0f, 1.0f);
    utils::draw_text(player_pos_buff, 10.0f, 10.0f);

    // FPS
    snprintf(fps_buff, sizeof(fps_buff), "FPS: %.2f", fps);
    glColor3f(1.0f, 1.0f, 1.0f);
    utils::draw_text(fps_buff, 10.0f, 30.0f);

    glfwSwapBuffers(window);
    glfwPollEvents();
  }

  glfwTerminate();
  return 0;
}
