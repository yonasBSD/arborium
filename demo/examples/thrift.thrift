// Apache Thrift IDL example
namespace py example.thrift
namespace java com.example.thrift
namespace rs example_thrift

enum Status {
    ACTIVE = 1,
    INACTIVE = 2,
    PENDING = 3,
}

struct User {
    1: required i64 id,
    2: required string username,
    3: optional string email,
    4: Status status = Status.ACTIVE,
    5: map<string, string> metadata,
    6: list<string> roles,
}

exception UserNotFound {
    1: i64 userId,
    2: string message,
}

service UserService {
    User getUser(1: i64 id) throws (1: UserNotFound notFound),
    list<User> listUsers(1: i32 limit, 2: i32 offset),
    void createUser(1: User user),
    bool deleteUser(1: i64 id),
    oneway void logAction(1: string action),
}
