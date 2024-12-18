#ifndef UTILS_H
#define UTILS_H

#include <string>

namespace utils {

void draw_text(const std::string& text, float x, float y);

void show_fps(double& delta_time, double& last_time, unsigned int& loops,
              char* fps_buff, size_t buffer_size);

}  // namespace utils

#endif