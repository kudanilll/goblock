#include "shader.h"

// #include <GL/glew.h>
#include <GLFW/glfw3.h>

#include <fstream>
#include <iostream>
#include <sstream>
#include <string>

GLuint load_shader(const char* vertex_path, const char* fragment_path) {
  std::string vertex_code;
  std::string fragment_code;
  std::ifstream v_shader_file, f_shader_file;

  v_shader_file.open(vertex_path);
  f_shader_file.open(fragment_path);
  std::stringstream v_shader_stream, f_shader_stream;

  v_shader_stream << v_shader_file.rdbuf();
  f_shader_stream << f_shader_file.rdbuf();

  v_shader_file.close();
  f_shader_file.close();

  vertex_code = v_shader_stream.str();
  fragment_code = f_shader_stream.str();

  const char* v_shader_code = vertex_code.c_str();
  const char* f_shader_code = fragment_code.c_str();

  GLuint vertex, fragment;
  GLint success;
  GLchar info_log[512];

  // Compile vertex shader
  vertex = glCreateShader(GL_VERTEX_SHADER);
  glShaderSource(vertex, 1, &v_shader_code, NULL);
  glCompileShader(vertex);
  glGetShaderiv(vertex, GL_COMPILE_STATUS, &success);
  if (!success) {
#ifdef __DEBUG_MODE__
    glGetShaderInfoLog(vertex, 512, NULL, info_log);
    std::cerr << "error: vertex shader compilation failed\n"
              << info_log << std::endl;
#endif
  }

  // Compile fragment shader
  fragment = glCreateShader(GL_FRAGMENT_SHADER);
  glShaderSource(fragment, 1, &f_shader_code, NULL);
  glCompileShader(fragment);
  glGetShaderiv(fragment, GL_COMPILE_STATUS, &success);
  if (!success) {
#ifdef __DEBUG_MODE__
    glGetShaderInfoLog(fragment, 512, NULL, info_log);
    std::cerr << "error: fragment shader compilation failed\n"
              << info_log << std::endl;
#endif
  }

  // Link shaders
  GLuint shader_program = glCreateProgram();
  glAttachShader(shader_program, vertex);
  glAttachShader(shader_program, fragment);
  glLinkProgram(shader_program);
  glGetProgramiv(shader_program, GL_LINK_STATUS, &success);
  if (!success) {
#ifdef __DEBUG_MODE__
    glGetProgramInfoLog(shader_program, 512, NULL, info_log);
    std::cerr << "error: shader program linking failed\n"
              << info_log << std::endl;
#endif
  }

  glDeleteShader(vertex);
  glDeleteShader(fragment);

  return shader_program;
}
