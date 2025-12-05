#include <iostream>
#include <vector>
#include <algorithm>

template<typename T>
T sum(const std::vector<T>& vec) {
    return std::accumulate(vec.begin(), vec.end(), T{});
}

int main() {
    std::vector<int> numbers = {1, 2, 3, 4, 5};
    std::cout << "Sum: " << sum(numbers) << std::endl;
    return 0;
}
