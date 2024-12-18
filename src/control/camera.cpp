#include "camera.h"

Camera::Camera()
    : position(glm::vec3(0.0f, 5.0f, 10.0f)),
      front(glm::vec3(0.0f, 0.0f, -1.0f)),
      up(glm::vec3(0.0f, 1.0f, 0.0f)),
      yaw(-90.0f),
      pitch(0.0f),
      movement_speed(10.0f),
      mouse_sensitivity(0.3f),
      fov(45.0f) {
  update_camera_vectors();
}

void Camera::update() {}

void Camera::process_keyboard(glm::vec3 direction, float deltaTime) {
  float velocity = movement_speed * deltaTime;
  position += direction * velocity;
}

void Camera::process_mouse_movement(float xoffset, float yoffset) {
  xoffset *= mouse_sensitivity;
  yoffset *= mouse_sensitivity;

  yaw += xoffset;
  pitch += yoffset;

  // Batasi sudut pandang vertikal agar tidak terbalik
  if (pitch > 89.0f) pitch = 89.0f;
  if (pitch < -89.0f) pitch = -89.0f;

  update_camera_vectors();
}

void Camera::update_camera_vectors() {
  glm::vec3 newFront;
  newFront.x = cos(glm::radians(yaw)) * cos(glm::radians(pitch));
  newFront.y = sin(glm::radians(pitch));
  newFront.z = sin(glm::radians(yaw)) * cos(glm::radians(pitch));
  front = glm::normalize(newFront);

  right = glm::normalize(glm::cross(front, up));
}

glm::mat4 Camera::get_view_matrix() const {
  return glm::lookAt(position, position + front, up);
}

glm::mat4 Camera::get_projection_matrix() const {
  return glm::perspective(glm::radians(fov), 800.0f / 600.0f, 0.1f, 100.0f);
}
