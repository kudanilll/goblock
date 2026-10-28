#include "control.h"

#include <glm/glm.hpp>

Control::Control(GLFWwindow* window, Camera& camera,
                 const std::vector<Block>& blocks, float block_size)
    : window(window),
      camera(camera),
      blocks(blocks),
      block_size(block_size),
      last_x(400),
      last_y(300),
      first_mouse(true),
      last_frame(0.0),
      sprint_speed(40.0f),
      vertical_speed(10.0f) {
  glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_DISABLED);
  last_frame = glfwGetTime();
}

void Control::handle_input(const std::vector<Block>& blocks) {
  double now = glfwGetTime();
  float delta_time = static_cast<float>(now - last_frame);
  last_frame = now;

  process_keyboard_input(blocks, delta_time);
  process_mouse_input();
}

void Control::process_keyboard_input(const std::vector<Block>& blocks,
                                     float delta_time) {
  bool sprinting = glfwGetKey(window, GLFW_KEY_LEFT_SHIFT) == GLFW_PRESS ||
                   glfwGetKey(window, GLFW_KEY_RIGHT_SHIFT) == GLFW_PRESS;

  // Build one direction vector so diagonal keys don't move faster.
  glm::vec3 move(0.0f);
  if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) move += camera.front;
  if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) move -= camera.front;
  if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) move -= camera.right;
  if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) move += camera.right;

  if (glm::length(move) > 0.0f) {
    float speed = sprinting ? sprint_speed : camera.get_movement_speed();
    camera.process_keyboard(move, delta_time, speed, blocks);
  }

  // Vertical movement (fly up/down).
  glm::vec3 vertical(0.0f);
  if (glfwGetKey(window, GLFW_KEY_SPACE) == GLFW_PRESS) vertical.y += 1.0f;
  if (glfwGetKey(window, GLFW_KEY_LEFT_CONTROL) == GLFW_PRESS ||
      glfwGetKey(window, GLFW_KEY_C) == GLFW_PRESS)
    vertical.y -= 1.0f;

  if (glm::length(vertical) > 0.0f) {
    camera.process_keyboard(vertical, delta_time, vertical_speed, blocks);
  }

  // Escape
  if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
    glfwSetWindowShouldClose(window, true);
  }
}

void Control::process_mouse_input() {
  double xpos, ypos;
  glfwGetCursorPos(window, &xpos, &ypos);

  if (first_mouse) {
    last_x = xpos;
    last_y = ypos;
    first_mouse = false;
  }

  float xoffset = static_cast<float>(xpos - last_x);
  float yoffset = static_cast<float>(last_y - ypos);  // Y terbalik

  last_x = xpos;
  last_y = ypos;

  camera.process_mouse_movement(xoffset, yoffset);
}
