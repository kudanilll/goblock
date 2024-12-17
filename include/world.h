#ifndef WORLD_H
#define WORLD_H

#include <vector>

struct Block {
  bool is_active;

  // 0: default
  // 1: tanah
  // 2: batu
  int type;
};

class World {
 public:
  World();

  int get_width();
  int get_height();
  int get_depth();

  Block get_block(int x, int y, int z);
  void set_block(int x, int y, int z, int type);

 private:
  const int WIDTH = 16;
  const int HEIGHT = 16;
  const int DEPTH = 16;

  std::vector<std::vector<std::vector<Block>>> blocks;
};

#endif
