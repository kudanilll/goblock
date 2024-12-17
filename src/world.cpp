#include "world.h"

World::World() {
  blocks.resize(get_width(),
                std::vector<std::vector<Block>>(
                    get_height(), std::vector<Block>(get_depth(), {false, 0})));
  for (int x = 0; x < get_width(); x++) {
    for (int z = 0; z < get_depth(); z++) {
      for (int y = 0; y < get_height() / 2; y++) {
        blocks[x][y][z] = {true, 1};
      }
    }
  }
}

int World::get_width() { return WIDTH; }

int World::get_height() { return HEIGHT; }

int World::get_depth() { return DEPTH; }

Block World::get_block(int x, int y, int z) {
  if (x >= 0 && x < get_width() && y >= 0 && y < get_height() && z >= 0 &&
      z < get_depth()) {
    return blocks[x][y][z];
  }
  return {false, 0};
}

void World::set_block(int x, int y, int z, int type) {
  if (x >= 0 && x < get_width() && y >= 0 && y < get_height() && z >= 0 &&
      z < get_depth()) {
    blocks[x][y][z] = {type != 0, type};
  }
}
