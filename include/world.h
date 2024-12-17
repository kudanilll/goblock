#ifndef WORLD_H
#define WORLD_H

#include <vector>

struct Block {
  bool isActive;

  // 0: default
  // 1: tanah
  // 2: batu
  int type;
};

class World {
 public:
  static const int WIDTH = 16;
  static const int HEIGHT = 16;
  static const int DEPTH = 16;

  World();

  Block getBlock(int x, int y, int z);
  void setBlock(int x, int y, int z, int type);

 private:
  std::vector<std::vector<std::vector<Block>>> blocks;
};

#endif
