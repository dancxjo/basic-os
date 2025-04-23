//! Reversible Persona Name Generator from UUIDs

use alloc::string::ToString;
use alloc::{fmt, format};
use alloc::{fmt::Display, string::String};
use uuid::Uuid;

const FIRST_NAMES_MASC: &[&str] = &["Harvy", "Homer", "Michael", "Edgar", "Jules"];
const FIRST_NAMES_FEM: &[&str] = &["Mai", "Li", "Lou", "J. K.", "Edna"];
const MIDDLE_NAMES: &[&str] = &["J.", "K.", "Lou", "Ann", "Lee"];
const SURNAMES: &[&str] = &[
    "Smith",
    "Simpson",
    "Hoover",
    "Ramirez",
    "McMichaelson",
    "Flannery",
];
const PLACE_NAMES: &[&str] = &["Chairingsburgh", "Netheridge", "Glint"];

#[derive(Debug)]
enum NameForm {
    Simple,             // Michael Ramirez
    MiddleInitial,      // Homer J. Simpson
    InitDotInitSurname, // J. K. Rowling
    HyphenatedMiddle,   // Michael-Lou Ramirez
    HyphenatedSurname,  // McMichaelson-Flannery
    OfPlace,            // Edgar of Glint
    SurnameFirst,       // Li Mai
}

#[derive(Debug)]
pub struct PersonaName {
    form: NameForm,
    given: String,
    middle: Option<String>,
    surname: String,
}

impl PersonaName {
    pub fn from_uuid(uuid: Uuid) -> Self {
        let bytes = uuid.as_bytes();

        // Determine form
        let form = match bytes[0] % 7 {
            0 => NameForm::Simple,
            1 => NameForm::MiddleInitial,
            2 => NameForm::InitDotInitSurname,
            3 => NameForm::HyphenatedMiddle,
            4 => NameForm::HyphenatedSurname,
            5 => NameForm::OfPlace,
            _ => NameForm::SurnameFirst,
        };

        let gender_bit = (bytes[1] & 0b1000_0000) != 0;
        let given = if gender_bit {
            FIRST_NAMES_MASC[(bytes[1] as usize) % FIRST_NAMES_MASC.len()].to_string()
        } else {
            FIRST_NAMES_FEM[(bytes[1] as usize) % FIRST_NAMES_FEM.len()].to_string()
        };

        let middle = match form {
            NameForm::MiddleInitial | NameForm::InitDotInitSurname | NameForm::HyphenatedMiddle => {
                Some(MIDDLE_NAMES[(bytes[2] as usize) % MIDDLE_NAMES.len()].to_string())
            }
            _ => None,
        };

        let surname_base = SURNAMES[(bytes[3] as usize) % SURNAMES.len()].to_string();

        let surname = match form {
            NameForm::HyphenatedSurname => {
                let alt = SURNAMES[(bytes[4] as usize) % SURNAMES.len()];
                format!("{}-{}", surname_base, alt)
            }
            NameForm::OfPlace => {
                let place = PLACE_NAMES[(bytes[4] as usize) % PLACE_NAMES.len()];
                format!("of {}", place)
            }
            _ => surname_base,
        };

        PersonaName {
            form,
            given,
            middle,
            surname,
        }
    }

    fn display(&self) -> String {
        match self.form {
            NameForm::Simple => format!("{} {}", self.given, self.surname),
            NameForm::MiddleInitial => {
                let m = self.middle.as_ref().unwrap();
                format!("{} {} {}", self.given, m, self.surname)
            }
            NameForm::InitDotInitSurname => {
                let m = self.middle.as_ref().unwrap();
                format!("{} {} {}", &self.given[..2], m, self.surname)
            }
            NameForm::HyphenatedMiddle => {
                let m = self.middle.as_ref().unwrap();
                format!("{}-{} {}", self.given, m, self.surname)
            }
            NameForm::HyphenatedSurname => format!("{} {}", self.given, self.surname),
            NameForm::OfPlace => format!("{} {}", self.given, self.surname),
            NameForm::SurnameFirst => format!("{} {}", self.surname, self.given),
        }
    }
}

impl fmt::Display for PersonaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}
