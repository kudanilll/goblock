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
double lastTime, deltaTime;
char aFPS[20];

// Fungsi untuk menggambar cube
void drawCube(float x, float y, float z) {
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

void calculateFPS() {
  double currentTime = glfwGetTime();
  deltaTime = currentTime - lastTime;
  loops++;
  if (deltaTime >= 1.0) {  // Update setiap detik
    snprintf(aFPS, sizeof(aFPS), "FPS: %d", loops);
    std::cout << "current fps: " << loops << std::endl;
    loops = 0;
    lastTime = currentTime;
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

  glm::vec3 cameraPos = glm::vec3(8.0f, 12.0f, 30.0f);   // Posisi kamera
  glm::vec3 cameraFront = glm::vec3(0.0f, 0.0f, -1.0f);  // Arah depan kamera
  glm::vec3 cameraUp = glm::vec3(0.0f, 1.0f, 0.0f);      // Arah "atas"

  float yaw = -90.0f;
  float pitch = 0.0f;
  float lastX = 400, lastY = 300;  // Posisi mouse awal
  bool firstMouse = true;
  float cameraSpeed = 0.10f;

  // View matrix
  glm::mat4 projection =
      glm::perspective(glm::radians(45.0f), 800.0f / 600.0f, 0.1f, 100.0f);
  glm::mat4 view;

  lastTime = glfwGetTime();

  while (!glfwWindowShouldClose(window)) {
    // float deltaTime = glfwGetTime();
    // glfwSetTime(0.0);

    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

    calculateFPS();

    // Input mouse
    double xpos, ypos;
    glfwGetCursorPos(window, &xpos, &ypos);

    if (firstMouse) {
      lastX = xpos;
      lastY = ypos;
      firstMouse = false;
    }

    float xoffset = xpos - lastX;
    float yoffset = lastY - ypos;
    lastX = xpos;
    lastY = ypos;

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
    cameraFront = glm::normalize(front);

    view = glm::lookAt(cameraPos, cameraPos + cameraFront, cameraUp);
    glMatrixMode(GL_PROJECTION);
    glLoadMatrixf(glm::value_ptr(projection));

    glMatrixMode(GL_MODELVIEW);
    glLoadMatrixf(glm::value_ptr(view));

    // Kontrol WASD
    if (glfwGetKey(window, GLFW_KEY_W) == GLFW_PRESS) {
      cameraPos += cameraSpeed * cameraFront;  // Maju
    }
    if (glfwGetKey(window, GLFW_KEY_S) == GLFW_PRESS) {
      cameraPos -= cameraSpeed * cameraFront;  // Mundur
    }
    if (glfwGetKey(window, GLFW_KEY_A) == GLFW_PRESS) {
      cameraPos -= glm::normalize(glm::cross(cameraFront, cameraUp)) *
                   cameraSpeed;  // Kiri
    }
    if (glfwGetKey(window, GLFW_KEY_D) == GLFW_PRESS) {
      cameraPos += glm::normalize(glm::cross(cameraFront, cameraUp)) *
                   cameraSpeed;  // Kanan
    }

    //  Exit
    if (glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS) {
      break;
    }

    // Render
    for (int x = 0; x < World::WIDTH; x++) {
      for (int y = 0; y < World::HEIGHT; y++) {
        for (int z = 0; z < World::DEPTH; z++) {
          Block block = world.getBlock(x, y, z);
          // if (block.isActive) {
          //   std::cout << "Rendering block at (" << x << ", " << y << ", "
          //   <<
          //   z
          //             << ")\n";
          //   drawCube(x, y, z);
          // } else {
          //   std::cout << "Block at (" << x << ", " << y << ", " << z
          //             << ") is inactive.\n";
          // }
          if (block.isActive) {
            drawCube(x, y, z);
          }
        }
      }
    }

    // Show FPS
    glColor3f(1.0f, 0.0f, 0.0f);
    drawText(aFPS, 10.0f, 10.0f);

    glfwSwapBuffers(window);
    glfwPollEvents();
  }

  glfwTerminate();
  return 0;
}
