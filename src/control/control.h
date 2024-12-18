#ifndef CONTROL_H
#define CONTROL_H

#include <GLFW/glfw3.h>

#include "../block/block.h"
#include "camera.h"

class Control {
 public:
  Control(GLFWwindow* window, Camera& camera, const std::vector<Block>& blocks,
          float block_size);

  void handle_input(const std::vector<Block>& blocks);

 private:
  GLFWwindow* window;
  Camera& camera;

  const std::vector<Block>& blocks;
  float block_size;
  float last_x, last_y;
  bool first_mouse;

  void process_keyboard_input(const std::vector<Block>& blocks);
  void process_mouse_input();
};

#endif
