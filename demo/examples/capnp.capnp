# Cap'n Proto schema
@0xdbb9ad1f14bf0b36;

struct Person {
  name @0 :Text;
  age @1 :UInt32;
  email @2 :Text;

  phones @3 :List(PhoneNumber);

  struct PhoneNumber {
    number @0 :Text;
    type @1 :Type;

    enum Type {
      mobile @0;
      home @1;
      work @2;
    }
  }
}

struct AddressBook {
  people @0 :List(Person);
}

interface PersonService {
  getPerson @0 (id :UInt64) -> (person :Person);
  addPerson @1 (person :Person) -> (id :UInt64);
  listPeople @2 () -> (people :List(Person));
}
