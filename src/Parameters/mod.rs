
use rand;
use std::io::BufReader;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::collections::{HashMap, HashSet};
use rand::Rng;
use ansi_term::Colour::{Yellow, Red};
use std::boxed::Box;
use std::mem;

#[derive(Clone,Debug,RustcEncodable)]
pub struct ParamsConfigurator {
    // Path of the file where the parameters configuration is
    pub param_file_path: String,
    // HashMap that stores the space state of each parameter
    pub params_space_state: HashMap<String, Vec<u32>>,
    // Indexes of parameters. It is needed to have an order of the parameters
    // for the insertion of new states into the visited_params_state.
    pub params_indexes: HashMap<String, u8>,
    // Visited parameters list. Saved in heap for memory space reasons
    pub visited_params_states: Box<HashSet<String>>,
}

static initial_decreasing_factor: f64 = 0.6;

impl ParamsConfigurator {
    pub fn new() -> ParamsConfigurator {
        Default::default()
    }


    /**
	Access the initial-params.conf file and extract the info on parameters to tune
	It returns the initial params state given in input by the user
	**/
    pub fn get_initial_param_conf(&mut self) -> HashMap<String, u32> {

        let f = self.param_file_path.clone();
        // Create a path to the desired file
        let path = Path::new(&f);
        let display = path.display();

        // Open the path in read-only mode, returns `io::Result<File>`
        let file = match File::open(&path) {
            Err(why) => panic!("couldn't open {}: {}", display, why.description()),
            Ok(file) => file,
        };

        let mut initial_params_state: HashMap<String, u32> = HashMap::new();
        let file_reader = BufReader::new(&file);
        let mut index = 0;
        for (_, line) in file_reader.lines().enumerate() {
            let topline = line.unwrap();
            let mut topsplit = topline.split(":");

            let (var_name, var_value, var_lbound, var_ubound, var_step);

            match topsplit.next() {
                Some(x) => var_name = x,
                None => break,
            }

            match topsplit.next() {
                Some(subline) => {
                    let mut subsplit = subline.split(",");
                    match subsplit.next() {
                        Some(x) => var_lbound = str::replace(x, "[", ""),
                        None => break,
                    }
                    match subsplit.next() {
                        Some(x) => var_ubound = x,
                        None => break,
                    }
                    match subsplit.next() {
                        Some(x) => var_step = str::replace(x, "]", ""),
                        None => break,		                		
                    }
                }
                None => break,
            }

            match topsplit.next() {
                Some(x) => var_value = x,
                None => break,
            }



            let space_state_elems =
                ParamsConfigurator::get_space_state(var_lbound.parse::<u32>().unwrap(),
                                                    var_ubound.parse::<u32>().unwrap(),
                                                    var_step.parse::<u32>().unwrap());
            let space_state_elems_c = space_state_elems.clone();

            self.params_space_state
                .insert(var_name.to_string(), space_state_elems);
            self.params_indexes.insert(var_name.to_string(), index);
            index = index + 1;

            initial_params_state.insert(var_name.to_string(), var_value.parse::<u32>().unwrap());


            println!("{} {:?}", Yellow.paint("Input Parameter ==> "), var_name);

            println!("{} [{:?},{:?},{:?}] - {} {:?} ",Yellow.paint("Space State ==> "),
                     var_lbound,
                     var_ubound,
                     var_step,
                     Yellow.paint("Default Value ==> "),
                     var_value,
                     );
            println!("{} {:?}",
                     Yellow.paint("Elements ==> "),
                     space_state_elems_c);

            println!("{}",Red.paint("*******************************************************************************************************************"));

        }

        return initial_params_state.clone();

    }


    /**
	Private function useful to generate the whole space state for each parameter based on the [min:max:step] values
	given in input by the user.
	**/
    fn get_space_state(lbound: u32, ubound: u32, step: u32) -> Vec<u32> {
        let mut res_vec = Vec::new();
        let num_it = (ubound - lbound) / step;
        for x in 0..num_it {
            res_vec.push(lbound + (step * x));
            if x == num_it - 1 {
                res_vec.push(lbound + (step * (x + 1)));
            }
        }
        // Randomize the order of vector elements
        rand::thread_rng().shuffle(&mut res_vec);
        return res_vec;
    }


    /**
	Function that returns a random neighborhood of the state given in input. The Neighborhood evaluation is performed in 
	an adaptive way. At the beginning of the Annealing the space of Neighborhoods will be large (60% of the parameters will vary).
	Then, the more the number of steps executed increase, the more the Neighborhood space gets smaller.   
	**/
    pub fn get_rand_neighborhood(&mut self,
                                 params_state: &HashMap<String, u32>,
                                 max_anneal_steps: u64,
                                 current_anneal_step: u64)
                                 -> Option<HashMap<String, u32>> {


        // Evaluate the coefficient with which decrease the size of neighborhood selection. The factor will
        // decrease every period_of_variation. The initial value of the factor has been set to 0.6. Therefore,
        // 60% of the parameters will vary at the beginning and then such a value will decrease of 10% every period
        let period_of_variation: f64 = max_anneal_steps as f64 /
                                       ((initial_decreasing_factor as f64) * 10.0);
        let decreasing_factor: f64 = initial_decreasing_factor -
                                     ((current_anneal_step as f64 / period_of_variation).floor()) /
                                     10.0;

        // Evaluate the number of varying parameters based on factor evaluated before
        let mut num_params_2_vary = (params_state.len() as f64 * decreasing_factor) as usize;

        let mut new_params_state: HashMap<String, u32> = HashMap::new();
        let mut state_4_history: Vec<u8> = vec!(0;params_state.len());

        // The HashMap iterator provides (key,value) pair in a random order
        for (param_name, param_current_value) in params_state.iter() {
            let current_space_state = self.params_space_state.get(param_name).unwrap();
            if num_params_2_vary > 0 {
                // If there are values that can be changed take
                let new_value = rand::thread_rng().choose(&current_space_state).unwrap();
                new_params_state.insert(param_name.clone().to_string(), *new_value);
                num_params_2_vary -= 1;
            } else {
                new_params_state.insert(param_name.clone().to_string(), *param_current_value);
            }

            // Put at the index extracted from the params_indexes the new state evaluated.
            // Note that it won't put the values of the state but its index into the space state vector.
            // This is for occupying less memory as possible.
            let index_in_space_state = current_space_state.iter()
                .position(|&r| r == *new_params_state.get(param_name).unwrap());
            match index_in_space_state {
                Some(i) => {
                    state_4_history[*self.params_indexes.get(param_name).unwrap() as usize] =
                        i as u8
                }
                None => panic!("I did not find the parameter into the space state!"),
            }

        }


        // Extract the string sequence of the new state
        let mut byte_state_str = String::new();
        for x in 0..state_4_history.len() {
            byte_state_str.push_str(&*state_4_history.get(x).unwrap().to_string());
        }

        state_4_history.clear();

        // Insert the new state into the visited hashmap. For memory efficiency the visited states parameters
        // values are coded through their index into the space_state vector.
        let there_wasnt = self.visited_params_states.insert(byte_state_str.clone());

        // If the neighborhood selected has been already visited recursively re-call the function
        // In case all states have been visited returns None to the Annealing Solver which will interrupt
        // the evaluation. Otherwise, the new state is added to the visited ones and the function return it.
        if there_wasnt == true {
            return Some(new_params_state);
        } else {
            return self.get_rand_neighborhood(params_state, max_anneal_steps, current_anneal_step);
        }
    }
}


impl Default for ParamsConfigurator {
    fn default() -> ParamsConfigurator {
        ParamsConfigurator {
            param_file_path: "".to_string(),
            params_space_state: HashMap::new(),
            params_indexes: HashMap::new(),
            visited_params_states: Box::new(HashSet::new()),
        }

    }
}
