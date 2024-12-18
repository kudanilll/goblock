#ifndef TEXT_H
#define TEXT_H

#include <glm/glm.hpp>
#include <string>
#include <vector>

#include "stb_truetype.h"

class Text {
 private:
  unsigned int shader_program;
  unsigned int texture;
  unsigned int vao, vbo;
  stbtt_fontinfo font;
  std::vector<unsigned char> buffer;
  int font_size;

  void compile_shader();

 public:
  Text(const std::string& fontPath, int fontSize = 24);
  ~Text();

  void render_text(const std::string& text, float x, float y,
                   float scale = 1.0f,
                   const glm::vec3& color = glm::vec3(1.0f, 1.0f, 1.0f));
};

#endif
