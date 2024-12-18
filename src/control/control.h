#ifndef CONTROL_H
#define CONTROL_H

#include <GLFW/glfw3.h>

#include "camera.h"

class Control {
 public:
  Control(GLFWwindow* window, Camera& camera);

  void handle_input();

 private:
  GLFWwindow* window;
  Camera& camera;

  float last_x, last_y;
  bool first_mouse;

  void process_keyboard_input();
  void process_mouse_input();
};

#endif
