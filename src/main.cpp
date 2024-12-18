#include <GL/glew.h>
#include <GLFW/glfw3.h>

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <iostream>

#include "control/camera.h"
#include "control/control.h"
#include "utils/utils.h"
#include "world/world.h"

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

int main() {
  const int window_width = 800;
  const int window_height = 600;

  GLFWwindow* window =
      initialize_window(window_width, window_height, "goblock");
  if (!window) return -1;

  // World
  World world;

  // Kamera & Kontrol
  Camera camera;
  Control control(window, camera);

  while (!glfwWindowShouldClose(window)) {
    // Input & Update
    control.handle_input();
    camera.update();

    // Render
    glClearColor(0.53f, 0.81f, 0.98f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    world.draw(camera.get_view_matrix(), camera.get_projection_matrix());

    glfwSwapBuffers(window);
    glfwPollEvents();
  }

  glfwTerminate();
  return 0;
}
