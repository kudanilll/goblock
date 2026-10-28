#include <GL/glew.h>
#include <GL/glut.h>
#include <GLFW/glfw3.h>
#include <stdio.h>

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <iostream>

#include "config.h"
#include "control/camera.h"
#include "control/control.h"
#include "gfx/shader.h"
#include "utils/color.h"
#include "utils/utils.h"
#include "world/world.h"

char player_pos_buff[0xFF];
char fps_buff[0xFF];
char sky_vs_buff[0xFF];
char morning_fs_buff[0xFF];
char day_fs_buff[0xFF];
char sunset_fs_buff[0xFF];
char night_fs_buff[0xFF];
char rainy_fs_buff[0xFF];
char aurora_fs_buff[0xFF];
char block_vs_buff[0xFF];
char block_fs_buff[0xFF];
// char sky_fs_buff[0xFF];

// FPS variable
double last_time = 0.0;
int frame_count = 0;
float fps = 0.0f;

// Shader variable
GLuint morning_shader;
GLuint day_shader;
GLuint sunset_shader;
GLuint night_shader;
GLuint rainy_shader;
GLuint aurora_shader;
GLuint current_shader;
GLuint block_shader;
float time_value;

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

void update_sky_shader() {
  float game_time = glfwGetTime();  // Atau waktu game Anda
  time_value = game_time;

  // Contoh logika pergantian waktu (dapat disesuaikan)
  int hour = ((int)(game_time / 10.0f)) % 24;  // 10 detik = 1 jam game

  if (hour >= 5 && hour < 10)
    current_shader = morning_shader;
  else if (hour >= 10 && hour < 15)
    current_shader = day_shader;
  else if (hour >= 15 && hour < 18)
    current_shader = sunset_shader;
  else
    current_shader = night_shader;

  // TODO: add logic for rainy and aurora
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

  // Load Shaders
  // snprintf(sky_fs_buff, sizeof(sky_fs_buff), "%s/sky.fs", SHADERS_DIR);
  snprintf(morning_fs_buff, sizeof(morning_fs_buff), "%s/sky/morning.fs",
           SHADERS_DIR);
  snprintf(day_fs_buff, sizeof(day_fs_buff), "%s/sky/day.fs", SHADERS_DIR);
  snprintf(sunset_fs_buff, sizeof(sunset_fs_buff), "%s/sky/sunset.fs",
           SHADERS_DIR);
  snprintf(night_fs_buff, sizeof(night_fs_buff), "%s/sky/night.fs",
           SHADERS_DIR);
  snprintf(rainy_fs_buff, sizeof(rainy_fs_buff), "%s/sky/rainy.fs",
           SHADERS_DIR);
  snprintf(aurora_fs_buff, sizeof(aurora_fs_buff), "%s/sky/aurora.fs",
           SHADERS_DIR);
  snprintf(sky_vs_buff, sizeof(sky_vs_buff), "%s/sky.vs", SHADERS_DIR);
  snprintf(block_vs_buff, sizeof(block_vs_buff), "%s/block.vs", SHADERS_DIR);
  snprintf(block_fs_buff, sizeof(block_fs_buff), "%s/block.fs", SHADERS_DIR);

#ifdef __DEBUG_MODE__
  // printf("load shader: %s\n", sky_fs_buff);
  printf("load shader: %s\n", morning_fs_buff);
  printf("load shader: %s\n", day_fs_buff);
  printf("load shader: %s\n", sunset_fs_buff);
  printf("load shader: %s\n", night_fs_buff);
  printf("load shader: %s\n", rainy_fs_buff);
  printf("load shader: %s\n", aurora_fs_buff);
  printf("load shader: %s\n", sky_vs_buff);
#endif

  // GLuint sky_shader = load_shader(sky_vs_buff, sky_fs_buff);
  morning_shader = load_shader(sky_vs_buff, morning_fs_buff);
  day_shader = load_shader(sky_vs_buff, day_fs_buff);
  sunset_shader = load_shader(sky_vs_buff, sunset_fs_buff);
  night_shader = load_shader(sky_vs_buff, night_fs_buff);
  rainy_shader = load_shader(sky_vs_buff, rainy_fs_buff);
  aurora_shader = load_shader(sky_vs_buff, aurora_fs_buff);
  block_shader = load_shader(block_vs_buff, block_fs_buff);

  // GLuint color_loc = glGetUniformLocation(sky_shader, "backgroundColor");
  float quad_vertices[] = {-1.0f, 1.0f,  0.0f, -1.0f, -1.0f, 0.0f,
                           1.0f,  -1.0f, 0.0f, 1.0f,  1.0f,  0.0f};

  unsigned int quad_indices[] = {0, 1, 2, 0, 2, 3};

  unsigned int sky_vao, sky_vbo, sky_ebo;
  glGenVertexArrays(1, &sky_vao);
  glGenBuffers(1, &sky_vbo);
  glGenBuffers(1, &sky_ebo);

  glBindVertexArray(sky_vao);
  glBindBuffer(GL_ARRAY_BUFFER, sky_vbo);
  glBufferData(GL_ARRAY_BUFFER, sizeof(quad_vertices), quad_vertices,
               GL_STATIC_DRAW);

  glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, sky_ebo);
  glBufferData(GL_ELEMENT_ARRAY_BUFFER, sizeof(quad_indices), quad_indices,
               GL_STATIC_DRAW);

  glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE, 3 * sizeof(float), (void*)0);
  glEnableVertexAttribArray(0);

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
    // glClearColor(0.53f, 0.81f, 0.98f, 1.0f);
    // glClearColor(RGBA_TO_GLCLAMPF(135, 206, 235, 255));
    // glUseProgram(sky_shader);
    // glUniform4f(color_loc, 0.53f, 0.81f, 0.98f, 1.0f);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

    // update_sky_shader();
    // glUseProgram(current_shader);
    glUseProgram(sunset_shader);

    // Update uniforms
    GLuint time_location = glGetUniformLocation(sunset_shader, "time");
    GLuint view_location = glGetUniformLocation(sunset_shader, "view");
    GLuint proj_location = glGetUniformLocation(sunset_shader, "projection");

    glUniform1f(time_location, time_value);
    glUniformMatrix4fv(view_location, 1, GL_FALSE,
                       glm::value_ptr(camera.get_view_matrix()));
    glUniformMatrix4fv(proj_location, 1, GL_FALSE,
                       glm::value_ptr(camera.get_projection_matrix()));

    glDepthMask(GL_FALSE);
    glBindVertexArray(sky_vao);
    glDrawElements(GL_TRIANGLES, 6, GL_UNSIGNED_INT, 0);
    glBindVertexArray(0);
    glDepthMask(GL_TRUE);

    // World
    glUseProgram(block_shader);
    glUniformMatrix4fv(glGetUniformLocation(block_shader, "view"), 1,
                       GL_FALSE,
                       glm::value_ptr(camera.get_view_matrix()));
    glUniformMatrix4fv(glGetUniformLocation(block_shader, "projection"), 1,
                       GL_FALSE,
                       glm::value_ptr(camera.get_projection_matrix()));
    glUniform3f(glGetUniformLocation(block_shader, "lightDir"), 0.4f, -1.0f,
                0.3f);
    glEnable(GL_CULL_FACE);
    glCullFace(GL_BACK);
    glFrontFace(GL_CCW);
    world.draw(camera.get_view_matrix(), camera.get_projection_matrix());
    glDisable(GL_CULL_FACE);
    glUseProgram(0);

#ifdef __DEBUG_MODE__
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
#endif

    glfwSwapBuffers(window);
    glfwPollEvents();


  }

  glDeleteVertexArrays(1, &sky_vao);
  glDeleteBuffers(1, &sky_vbo);
  glDeleteBuffers(1, &sky_ebo);
  glDeleteProgram(morning_shader);
  glDeleteProgram(day_shader);
  glDeleteProgram(sunset_shader);
  glDeleteProgram(night_shader);
  glDeleteProgram(aurora_shader);
  glDeleteProgram(rainy_shader);
  glfwTerminate();
  return 0;
}
