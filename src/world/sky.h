#ifndef SKY_H
#define SKY_H

enum SkyState { DAY, NIGHT, SUNRISE, SUNSET };

class Sky {
 private:
  float fog_near, fog_far;
  SkyState state;

 public:
  Sky();
  ~Sky();
};

#endif