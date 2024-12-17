#include <GL/glew.h>
#include <GL/glut.h>
#include <GLFW/glfw3.h>
#include <stdio.h>

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <iostream>

#include "utils.h"
#include "world.h"

unsigned int loops;
double last_time, delta_time;
char fps_buff[20];

// Fungsi untuk menggambar cube
void draw_cube(float x, float y, float z) {
  float size = 1.0f;
  glBegin(GL_QUADS);

  // Depan
  glColor3f(1.0f, 0.0f, 0.0f);  // Merah
  glVertex3f(x, y, z);
  glVertex3f(x + size, y, z);
  glVertex3f(x + size, y + size, z);
  glVertex3f(x, y + size, z);

  // Belakang
  glColor3f(0.0f, 1.0f, 0.0f);  // Hijau
  glVertex3f(x, y, z + size);
  glVertex3f(x + size, y, z + size);
  glVertex3f(x + size, y + size, z + size);
  glVertex3f(x, y + size, z + size);

  // Kiri
  glColor3f(0.0f, 0.0f, 1.0f);  // Biru
  glVertex3f(x, y, z);
  glVertex3f(x, y, z + size);
  glVertex3f(x, y + size, z + size);
  glVertex3f(x, y + size, z);

  // Kanan
  glColor3f(1.0f, 1.0f, 0.0f);  // Kuning
  glVertex3f(x + size, y, z);
  glVertex3f(x + size, y, z + size);
  glVertex3f(x + size, y + size, z + size);
  glVertex3f(x + size, y + size, z);

  // Atas
  glColor3f(1.0f, 0.0f, 1.0f);  // Ungu
  glVertex3f(x, y + size, z);
  glVertex3f(x + size, y + size, z);
  glVertex3f(x + size, y + size, z + size);
  glVertex3f(x, y + size, z + size);

  // Bawah
  glColor3f(0.0f, 1.0f, 1.0f);  // Cyan
  glVertex3f(x, y, z);
  glVertex3f(x + size, y, z);
  glVertex3f(x + size, y, z + size);
  glVertex3f(x, y, z + size);

  glEnd();
}

void calculate_fps() {
  double currentTime = glfwGetTime();
  delta_time = currentTime - last_time;
  loops++;
  if (delta_time >= 1.0) {  // Update setiap detik
    snprintf(fps_buff, sizeof(fps_buff), "FPS: %d", loops);
    std::cout << "current fps: " << loops << std::endl;
    loops = 0;
    last_time = currentTime;
  }
}

int main(int argc, char** argv) {
  glutInit(&argc, argv);

  if (!glfwInit()) {
    std::cerr << "Failed to initialize GLFW!" << std::endl;
    return -1;
  }

  GLFWwindow* window = glfwCreateWindow(800, 600, "GoBlock", nullptr, nullptr);
  if (!window) {
    std::cerr << "Failed to create GLFW window!" << std::endl;
    glfwTerminate();
    return -1;
  }

  glfwMakeContextCurrent(window);
  glfwSetInputMode(window, GLFW_CURSOR, GLFW_CURSOR_DISABLED);
  glewInit();
  glClearColor(0.5f, 0.7f, 1.0f, 1.0f);

  World world;

  // OpenGL setup
  glEnable(GL_DEPTH_TEST);
  glDepthFunc(GL_LESS);

  glm::vec3 camera_position = glm::vec3(8.0f, 12.0f, 30.0f);  // Posisi kamera
  glm::vec3 camera_front = glm::vec3(0.0f, 0.0f, -1.0f);  // Arah depan kamera
  glm::vec3 camera_up = glm::vec3(0.0f, 1.0f, 0.0f);      // Arah "atas"

  float yaw = -90.0f;
  float pitch = 0.0f;
  float last_x_mouse = 400, last_y_mouse = 300;  // Posisi mouse awal
  bool first_mouse = true;
  float camera_speed = 0.10f;

  // View matrix
  glm::mat4 projection =
      glm::perspective(glm::radians(45.0f), 800.0f / 600.0f, 0.1f, 100.0f);
  glm::mat4 view;

  last_time = glfwGetTime();

  while (!glfwWindowShouldClose(window)) {
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

    calculate_fps();

    // Input mouse
    double xpos, ypos;
    glfwGetCursorPos(window, &xpos, &ypos);

    if (first_mouse) {
      last_x_mouse = xpos;
      last_y_mouse = ypos;
      first_mouse = false;
    }

    float xoffset = xpos - last_x_mouse;
    float yoffset = last_y_mouse - ypos;
    last_x_mouse = xpos;
    last_y_mouse = ypos;

    float sensitivity = 0.05f;
    xoffset *= sensitivity;
    yoffset *= sensitivity;

    yaw += xoffset;
    pitch += yoffset;

    if (pitch > 89.0f) pitch = 89.0f;
    if (pitch < -89.0f) pitch = -89.0f;

    // Hitung arah depan kamera
    glm::vec3 front;
    front.x = cos(glm::radians(yaw)) * cos(glm::radians(pitch));
    front.y = sin(glm::radians(pitch));
    front.z = sin(glm::radians(yaw)) * cos(glm::radians(pitch));
    camera_front = glm::normalize(front);

    view =
        glm::lookAt(camera_position, camera_position + camera_front, camera_up);
    glMatrixMode(GL_PROJECTION);
    glLoadMatrixf(glm::value_ptr(projection));

    glMatrixMode(GL_MODELVIEW);
    glLoadMatrixf(glm::value_ptr(view));

    // Kontrol WASD
    if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) {
      camera_position += camera_speed * camera_front;  // Maju
    }
    if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) {
      camera_position -= camera_speed * camera_front;  // Mundur
    }
    if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) {
      camera_position -= glm::normalize(glm::cross(camera_front, camera_up)) *
                         camera_speed;  // Kiri
    }
    if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) {
      camera_position += glm::normalize(glm::cross(camera_front, camera_up)) *
                         camera_speed;  // Kanan
    }

    //  Exit
    if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
      break;
    }

    // Render
    for (int x = 0; x < world.get_width(); x++) {
      for (int y = 0; y < world.get_height(); y++) {
        for (int z = 0; z < world.get_depth(); z++) {
          Block block = world.get_block(x, y, z);
          // if (block.is_active) {
          //   std::cout << "Rendering block at (" << x << ", " << y << ", "
          //   <<
          //   z
          //             << ")\n";
          //   draw_cube(x, y, z);
          // } else {
          //   std::cout << "Block at (" << x << ", " << y << ", " << z
          //             << ") is inactive.\n";
          // }
          if (block.is_active) {
            draw_cube(x, y, z);
          }
        }
      }
    }

    // Show FPS
    glColor3f(1.0f, 0.0f, 0.0f);
    draw_text(fps_buff, 10.0f, 10.0f);

    glfwSwapBuffers(window);
    glfwPollEvents();
  }

  glfwTerminate();
  return 0;
}
