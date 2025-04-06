struct AnimalList{
   list: Vec<Box<SpecieSpecificAnimalList<dyn HasAdditionalFields>>>
}

struct SpecieSpecificAnimalList<T: HasAdditionalFields + ?Sized>{
    list: Vec<Box<T>>
}

struct Animal<T: HasAdditionalFields + ?Sized>{
    name: String,
    additional_fields: T
}

trait HasAdditionalFields {
    fn list_fields(&self);
}

trait Cats{}

impl HasAdditionalFields for dyn Cats {
    fn list_fields(&self) {
        todo!()
    }
}

trait Insects{}

impl HasAdditionalFields for dyn Insects {
    fn list_fields(&self) {
        todo!()
    }
}