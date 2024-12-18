#ifndef CAMERA_H
#define CAMERA_H

#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>

class Camera {
 public:
  glm::vec3 position;
  glm::vec3 front;
  glm::vec3 up;
  glm::vec3 right;

  Camera();

  void update();
  void process_mouse_movement(float xoffset, float yoffset);
  void process_keyboard(glm::vec3 direction, float deltaTime);

  glm::mat4 get_view_matrix() const;
  glm::mat4 get_projection_matrix() const;

 private:
  float yaw;
  float pitch;

  float movement_speed;
  float mouse_sensitivity;
  float fov;

  void update_camera_vectors();
};

#endif
