#include <stdio.h>
#include <stdlib.h>

typedef struct {
    int x, y;
} Point;

int main(int argc, char *argv[]) {
    Point p = {10, 20};
    printf("Point: (%d, %d)\n", p.x, p.y);
    return 0;
}
