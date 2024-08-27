use frontend::AppContext;
use yew::{
    context::ContextProvider, function_component, html, use_context, use_state, Callback, Html,
    Properties,
};

#[function_component]
fn CurrentUser() -> Html {
    let ctx = use_context::<AppContext>().expect("app context not found");

    match ctx.current_user {
        Some(user) => html!(<p>{format!("Current User: {}", user.email())}</p>),
        None => html!(<p>{"Not signed in."}</p>),
    }
}

#[derive(Properties, PartialEq)]
struct CallbackProps {
    on_click: Callback<()>,
}

#[function_component]
fn InteractiveComponent(CallbackProps { on_click }: &CallbackProps) -> Html {
    let click_handler = {
        let on_click = on_click.clone();
        Callback::from(move |_| on_click.emit(()))
    };

    html!(<button onclick={click_handler}>{"click me"}</button>)
}

#[function_component]
fn SignInForm() -> Html {
    html! {
        // Sign-in form template adapted from: https://tailwindui.com/components/application-ui/forms/sign-in-forms, accessed 22/08/2024
        <div class="flex min-h-full flex-col justify-center px-6 py-12 lg:px-8">
            <div class="sm:mx-auto sm:w-full sm:max-w-sm">
              <h2 class="mt-10 text-center text-2xl font-bold leading-9 tracking-tight text-gray-900">{"Sign in to your account"}</h2>
            </div>

            <div class="mt-10 sm:mx-auto sm:w-full sm:max-w-sm">
                <form class="space-y-6" action="#" method="POST">
                    <div>
                        <label for="email" class="block text-sm font-medium leading-6 text-gray-900">{"Email address"}</label>
                        <div class="mt-2">
                            <input id="email" name="email" type="email" autocomplete="email" required=true class="block w-full rounded-md border-0 py-1.5 text-gray-900 shadow-sm ring-1 ring-inset ring-gray-300 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600 sm:text-sm sm:leading-6"/>
                        </div>
                    </div>

                    <div>
                        <div class="flex items-center justify-between">
                            <label for="password" class="block text-sm font-medium leading-6 text-gray-900">{"Password"}</label>
                            <div class="text-sm">
                            <a href="#" class="font-semibold text-indigo-600 hover:text-indigo-500">{"Forgot password?"}</a>
                            </div>
                        </div>
                        <div class="mt-2">
                            <input id="password" name="password" type="password" autocomplete="current-password" required=true class="block w-full rounded-md border-0 py-1.5 text-gray-900 shadow-sm ring-1 ring-inset ring-gray-300 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600 sm:text-sm sm:leading-6"/>
                        </div>
                    </div>

                    <div>
                        <button type="submit" class="flex w-full justify-center rounded-md bg-indigo-600 px-3 py-1.5 text-sm font-semibold leading-6 text-white shadow-sm hover:bg-indigo-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">{"Sign in"}</button>
                    </div>
                </form>

                <p class="mt-10 text-center text-sm text-gray-500">
                  {"Don't have an account? "}
                  <a href="#" class="font-semibold leading-6 text-indigo-600 hover:text-indigo-500">{"Register here"}</a>
                </p>
            </div>
        </div>
    }
}

#[function_component]
fn App() -> Html {
    let ctx = use_state(|| AppContext {
        current_user: None,
        token: None,
    });

    let greeting = use_state(|| None);

    let sign_in_callback = {
        let greeting = greeting.clone();

        Callback::from(move |_| {
            match *greeting {
                Some(_) => greeting.set(None),
                None => greeting.set(Some("Hello, world!".to_string())),
            };
        })
    };

    html! {
        <div class="container">
            <ContextProvider<AppContext> context={(*ctx).clone()}>
                <SignInForm />
                <CurrentUser />

                <InteractiveComponent on_click={sign_in_callback.clone()} />

                if let Some(greeting_text) = greeting.as_ref() {
                    <p>{greeting_text}</p>
                }
            </ContextProvider<AppContext>>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
