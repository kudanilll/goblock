#include "world.h"

World::World() {
  blocks.resize(WIDTH, std::vector<std::vector<Block>>(
                           HEIGHT, std::vector<Block>(DEPTH, {false, 0})));
  for (int x = 0; x < WIDTH; x++) {
    for (int z = 0; z < DEPTH; z++) {
      for (int y = 0; y < HEIGHT / 2; y++) {
        blocks[x][y][z] = {true, 1};  // Blok tipe 1 (tanah)
      }
    }
  }
}

Block World::getBlock(int x, int y, int z) {
  if (x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT && z >= 0 && z < DEPTH) {
    return blocks[x][y][z];
  }
  return {false, 0};  // Blok kosong
}

void World::setBlock(int x, int y, int z, int type) {
  if (x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT && z >= 0 && z < DEPTH) {
    blocks[x][y][z] = {type != 0, type};
  }
}
