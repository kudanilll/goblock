#include "control/control.h"

Control::Control(GLFWwindow* window, Camera& camera)
    : window(window),
      camera(camera),
      last_x(400),
      last_y(300),
      first_mouse(true) {
  glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_DISABLED);
}

void Control::handle_input() {
  process_keyboard_input();
  process_mouse_input();
}

void Control::process_keyboard_input() {
  float deltaTime = 0.01f;  // Set time step kecil untuk gerakan konstan

  // WASD
  if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) {
    camera.process_keyboard(camera.front, deltaTime);
  }
  if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) {
    camera.process_keyboard(-camera.front, deltaTime);
  }
  if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) {
    camera.process_keyboard(-camera.right, deltaTime);
  }
  if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) {
    camera.process_keyboard(camera.right, deltaTime);
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

  float xoffset = xpos - last_x;
  float yoffset = last_y - ypos;  // Y terbalik

  last_x = xpos;
  last_y = ypos;

  camera.process_mouse_movement(xoffset, yoffset);
}
