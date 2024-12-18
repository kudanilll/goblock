#define STB_TRUETYPE_IMPLEMENTATION

#include "text.h"

#include <GL/glew.h>

#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>
#include <iostream>

Text::Text(const std::string& fontPath, int fontSize) : font_size(fontSize) {
  // Baca file font
  FILE* font_file = fopen(fontPath.c_str(), "rb");
  if (!font_file) {
    std::cerr << "Failed to open font file: " << fontPath << std::endl;
    throw std::runtime_error("Font file not found");
  }

  // Dapatkan ukuran file
  fseek(font_file, 0, SEEK_END);
  long file_size = ftell(font_file);
  fseek(font_file, 0, SEEK_SET);

  // Alokasi buffer
  buffer.resize(file_size);
  fread(buffer.data(), 1, file_size, font_file);
  fclose(font_file);

  // Inisialisasi font
  if (!stbtt_InitFont(&font, buffer.data(), 0)) {
    std::cerr << "Failed to initialize font!" << std::endl;
    throw std::runtime_error("Font initialization failed");
  }

  // Compile shader
  compile_shader();

  // Setup VAO dan VBO untuk rendering
  glGenVertexArrays(1, &vao);
  glGenBuffers(1, &vbo);
  glBindVertexArray(vao);
  glBindBuffer(GL_ARRAY_BUFFER, vbo);

  glEnableVertexAttribArray(0);
  glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float), (void*)0);
  glEnableVertexAttribArray(1);
  glVertexAttribPointer(1, 2, GL_FLOAT, GL_FALSE, 4 * sizeof(float),
                        (void*)(2 * sizeof(float)));

  // Setup texture
  glGenTextures(1, &texture);
  glBindTexture(GL_TEXTURE_2D, texture);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
  glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
}

Text::~Text() {
  glDeleteVertexArrays(1, &vao);
  glDeleteBuffers(1, &vbo);
  glDeleteTextures(1, &texture);
  glDeleteProgram(shader_program);
}

void Text::compile_shader() {
  // Vertex shader
  const char* vertex_shader_source = R"(
        #version 330 core
        layout (location = 0) in vec4 vertex; // <vec2 pos, vec2 tex>
        out vec2 TexCoords;
        uniform mat4 projection;
        void main() {
            gl_Position = projection * vec4(vertex.xy, 0.0, 1.0);
            TexCoords = vertex.zw;
        }
    )";

  // Fragment shader
  const char* fragment_shader_source = R"(
        #version 330 core
        in vec2 TexCoords;
        out vec4 color;
        uniform sampler2D text;
        uniform vec3 textColor;
        void main() {    
          float alpha = texture(text, TexCoords).a;
          color = vec4(textColor, alpha);
        }
    )";

  // Compile vertex shader
  unsigned int vertex_shader = glCreateShader(GL_VERTEX_SHADER);
  glShaderSource(vertex_shader, 1, &vertex_shader_source, NULL);
  glCompileShader(vertex_shader);

  // Compile fragment shader
  unsigned int fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
  glShaderSource(fragment_shader, 1, &fragment_shader_source, NULL);
  glCompileShader(fragment_shader);

  // Link shader program
  shader_program = glCreateProgram();
  glAttachShader(shader_program, vertex_shader);
  glAttachShader(shader_program, fragment_shader);
  glLinkProgram(shader_program);

  // Cleanup
  glDeleteShader(vertex_shader);
  glDeleteShader(fragment_shader);
}

void Text::render_text(const std::string& text, float x, float y, float scale,
                       const glm::vec3& color) {
  // Gunakan shader program
  glUseProgram(shader_program);

  // Dapatkan ukuran viewport saat ini
  GLint viewport[4];
  glGetIntegerv(GL_VIEWPORT, viewport);
  glm::mat4 projection = glm::ortho(0.0f, static_cast<float>(viewport[2]), 0.0f,
                                    static_cast<float>(viewport[3]));
  glUniformMatrix4fv(glGetUniformLocation(shader_program, "projection"), 1,
                     GL_FALSE, glm::value_ptr(projection));

  // Set warna teks
  glUniform3f(glGetUniformLocation(shader_program, "textColor"), color.x,
              color.y, color.z);

  glActiveTexture(GL_TEXTURE0);
  glBindVertexArray(vao);

  float current_x = x;
  for (char c : text) {
    int codepoint = c;
    int advance_width, left_side_bearing;

    // Dapatkan metrik karakter
    stbtt_GetCodepointHMetrics(&font, codepoint, &advance_width,
                               &left_side_bearing);

    // Hitung skala
    float scale_x = stbtt_ScaleForPixelHeight(&font, font_size) * scale;
    float scale_y = stbtt_ScaleForPixelHeight(&font, font_size) * scale;

    // Dapatkan dimensi bitmap
    int x0, y0, x1, y1;
    stbtt_GetCodepointBitmapBox(&font, codepoint, scale_x, scale_y, &x0, &y0,
                                &x1, &y1);

    int char_width = x1 - x0;
    int char_height = y1 - y0;

    // Buat bitmap untuk karakter
    std::vector<unsigned char> char_bitmap(char_width * char_height);
    stbtt_MakeCodepointBitmap(&font, char_bitmap.data(), char_width,
                              char_height, char_width, scale_x, scale_y,
                              codepoint);

    // Update texture
    // PENTING: Gunakan GL_ALPHA atau GL_RED sesuai dengan format bitmap Anda
    glBindTexture(GL_TEXTURE_2D, texture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_ALPHA, char_width, char_height, 0,
                 GL_ALPHA, GL_UNSIGNED_BYTE, char_bitmap.data());

    // Hitung posisi vertex
    float x_pos = current_x + x0;
    float y_pos = y + y0;
    float w = char_width;
    float h = char_height;

    // Vertex data
    float vertices[6][4] = {
        {x_pos, y_pos + h, 0.0f, 0.0f},    {x_pos, y_pos, 0.0f, 1.0f},
        {x_pos + w, y_pos, 1.0f, 1.0f},

        {x_pos, y_pos + h, 0.0f, 0.0f},    {x_pos + w, y_pos, 1.0f, 1.0f},
        {x_pos + w, y_pos + h, 1.0f, 0.0f}};

    // Update VBO
    glBindBuffer(GL_ARRAY_BUFFER, vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(vertices), vertices, GL_DYNAMIC_DRAW);

    // Render
    glDrawArrays(GL_TRIANGLES, 0, 6);

    // Geser posisi X
    current_x += advance_width * scale_x;
  }

  // Cleanup
  glBindVertexArray(0);
  glBindTexture(GL_TEXTURE_2D, 0);
  glUseProgram(0);
}